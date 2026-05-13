use clap::Parser;
use csv::StringRecord;
use std::error::Error;
use std::io;

use gwas_utils::{get_delimeter_from_cli_argument, open_reader, open_writer};

pub(crate) const USAGE: &str =
    "gu csv_concat_files -i infile1.csv[.gz] infile2.csv[.gz] ... -o outfile.csv[.gz]";
pub(crate) const ABOUT: &str = "Concatenate multiple CSV files into a single file";

#[derive(Parser, Debug)]
pub(crate) struct Args {
    /// CSV files to concatenate (can be gzipped if filenames end with .gz)
    #[arg(short, long, num_args = 1.., required = true)]
    input: Vec<String>,

    /// Delimiter for CSV file reading and writing
    #[arg(short, long, default_value = "auto")]
    delim: String,

    /// Concatenated CSV file to write (will be gzipped if filename ends with .gz)
    #[arg(short, long, default_value = "stdout")]
    output: String,
}

pub(crate) fn run(args: Args) -> Result<(), Box<dyn Error>> {
    let (ifilenames, file_wtr, sep) = handle_commandline_args(args)?;
    process_files(ifilenames, file_wtr, sep)?;
    Ok(())
}

fn handle_commandline_args(
    args: Args,
) -> Result<(Vec<String>, gwas_utils::Writer, char), Box<dyn Error>> {
    let file_wtr: gwas_utils::Writer = open_writer(&args.output)?;
    let sep = match args.delim.as_str() {
        "auto" => {
            let seps = args
                .input
                .iter()
                .map(|filename| {
                    let mut file_rdr = open_reader(filename)?;
                    file_rdr.sniff()
                })
                .collect::<Result<Vec<char>, Box<dyn Error>>>()?;
            if seps.iter().all(|&s| s == seps[0]) {
                seps[0]
            } else {
                return Err("Inconsistent delimiters across input files".into());
            }
        }
        _ => get_delimeter_from_cli_argument(&args.delim)?,
    };
    Ok((args.input, file_wtr, sep))
}

fn process_files<W>(ifilenames: Vec<String>, wtr: W, sep: char) -> Result<(), Box<dyn Error>>
where
    W: io::Write,
{
    let mut csv_wtr = csv::WriterBuilder::new()
        .delimiter(sep as u8)
        .from_writer(wtr);

    let mut index_header = StringRecord::new();

    for (i, filename) in ifilenames.iter().enumerate() {
        let file_rdr: gwas_utils::Reader = open_reader(filename)?;
        let mut csv_rdr = csv::ReaderBuilder::new()
            .has_headers(true)
            .delimiter(sep as u8)
            .from_reader(file_rdr);
        if i == 0 {
            index_header = csv_rdr.headers()?.clone();
            csv_wtr.write_record(&index_header)?;
        } else {
            let header = csv_rdr.headers()?.clone();
            if header != index_header {
                return Err("Mismatched headers in input files".into());
            }
        }
        for result in csv_rdr.records() {
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
    use std::io::Cursor;

    use super::*;

    #[test]
    fn test_concat_files() {
        let ifilenames = vec![
            "testdata/small.1.regenie".to_string(),
            "testdata/small.2.regenie".to_string(),
        ];
        let mut wtr = Cursor::new(Vec::new());
        process_files(ifilenames, &mut wtr, ' ').unwrap();
        let desired_result = fs::read_to_string("testdata/small.concat.regenie").unwrap();
        let result = String::from_utf8(wtr.into_inner()).unwrap();
        assert_eq!(result, desired_result);
    }
}
