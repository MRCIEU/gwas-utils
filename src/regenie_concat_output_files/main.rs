use clap::Parser;
use csv::StringRecord;
use std::error::Error;
use std::process;

use gwas_utilities::{open_reader, open_writer};

const USAGE: &str = "regenie_concat_output_files -i <infile1.regenie[.gz] infile2.regenie[.gz] ...> -o outfile.regenie[.gz]";

#[derive(Parser, Debug)]
#[command(version, 
    about, 
    long_about = "Concatenates multiple Regenie output files into a single file. The input files must have the same header line (the first line of the file), and the output file will contain that header line followed by all the records from the input files. The input files can be gzipped if their filenames end with .gz, and the output file will be gzipped if its filename ends with .gz.",
    override_usage = USAGE
)]
struct Args {
    /// Regenie output files to concatenate (can be gzipped if filename ends with .gz)
    #[arg(short, long)]
    inputs: Vec<String>,

    /// Concatenated output file to write (will be gzipped if filename ends with .gz)
    #[arg(short, long)]
    output: String,
}

fn main() {
    if let Err(err) = run() {
        println!("Error: {}", err);
        println!("Usage: {}", USAGE);
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let (ifilenames, file_wtr) = handle_command_line_args()?;
    process_files(ifilenames, file_wtr)?;
    Ok(())
}

fn handle_command_line_args() -> Result<(Vec<String>, gwas_utilities::Writer), Box<dyn Error>> {
    let args = Args::parse();
    if args.inputs.len() < 2 {
        return Err("expected at least two input filenames provided to -i/--inputs".into())
    }
    let file_wtr: gwas_utilities::Writer = open_writer(&args.output)?;
    Ok((args.inputs, file_wtr))
}

fn process_files<W>(ifilenames: Vec<String>, wtr: W) -> Result<(), Box<dyn Error>> 
where W: std::io::Write
{
    let mut csv_wtr = csv::WriterBuilder::new()
        .delimiter(b' ')
        .from_writer(wtr);

    let mut index_header= StringRecord::new();

    for (i, filename) in ifilenames.iter().enumerate() {
        let file_rdr: gwas_utilities::Reader = open_reader(filename)?;
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(true)
            .delimiter(b' ')
            .from_reader(file_rdr);
        if i == 0 {
            index_header = rdr.headers()?.clone();
            csv_wtr.write_record(&index_header)?;
        } else {
            let header = rdr.headers()?.clone();
            if header != index_header {
                return Err("mismatched headers in input files".into())
            }
        }
        for result in rdr.records() {
            let record = result?;
            csv_wtr.write_record(&record)?;
        }
    }
    
    csv_wtr.flush()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn concat_files() {
        let ifilenames = vec!["testdata/small.1.regenie".to_string(), "testdata/small.2.regenie".to_string()];
        let mut wtr = std::io::Cursor::new(Vec::new());
        process_files(ifilenames, &mut wtr).unwrap();
        let desired_result = fs::read_to_string("testdata/small.concat.regenie").unwrap();
        let result= String::from_utf8(wtr.into_inner()).unwrap();
        assert_eq!(result, desired_result);
    }
}
