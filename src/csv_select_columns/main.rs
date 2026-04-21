use clap::Parser;
use std::error::Error;
use std::io;
use std::process;

use gwas_utilities::{get_delimeter, open_reader, open_writer};

const USAGE: &str = "csv_select_columns -i infile.csv[.gz] -d \" \" -c <column1 column2 ...> -o outfile.csv[.gz]";

#[derive(Parser, Debug)]
#[command(version, override_usage = USAGE, about = "Select specific columns from a CSV file.")]
struct Args {
    /// Input CSV file to process (can be gzipped if filename ends with .gz)
    #[arg(short, long)]
    input: String,

    /// Column names to select
    #[arg(short, long, num_args = 1..)]
    columns: Vec<String>,

    /// Delimiter for CSV file reading and writing (default is tab, use " " for space, etc.)
    #[arg(short, long, default_value = "\\t")]
    delim: String,

    /// Output file to write with selected columns (will be gzipped if filename ends with .gz)
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
    let (file_rdr, file_wtr, columns_to_select, sep) = handle_commandline_args()?;
    process_file(file_rdr, file_wtr, columns_to_select, sep)
}

fn handle_commandline_args()
-> Result<(gwas_utilities::Reader, gwas_utilities::Writer, Vec<String>, char), Box<dyn Error>> {
    let args = Args::parse();
    let file_rdr = open_reader(&args.input)?;
    let file_wtr = open_writer(&args.output)?;
    let sep = get_delimeter(&args.delim)?;
    Ok((file_rdr, file_wtr, args.columns, sep))
}

fn process_file<R, W>(rdr: R, wtr: W, columns_to_select: Vec<String>, sep: char) -> Result<(), Box<dyn Error>>
where
    R: io::Read,
    W: io::Write,
{
    let mut csv_rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .delimiter(sep as u8)
        .from_reader(rdr);

    let header = csv_rdr.headers()?.clone();

    let column_indices_to_retain: Vec<usize> = header
        .iter()
        .enumerate()
        .filter_map(|(i, s)| columns_to_select.contains(&s.to_string()).then_some(i))
        .collect();

    let header_reduced = column_indices_to_retain.iter().map(|&i| &header[i]).collect::<Vec<_>>();
    if header_reduced.is_empty() {
        return Err("No matching columns found".into());
    }

    let mut csv_wtr = csv::WriterBuilder::new().delimiter(sep as u8).from_writer(wtr);

    csv_wtr.write_record(&header_reduced)?;

    for result in csv_rdr.records() {
        let record = result?;
        let record_reduced = column_indices_to_retain.iter().map(|&i| &record[i]).collect::<Vec<_>>();
        csv_wtr.write_record(&record_reduced)?;
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
    fn test_get_delimiter() {
        assert_eq!(get_delimeter("\t").unwrap(), '\t');
        assert_eq!(get_delimeter("\\t").unwrap(), '\t');
        assert_eq!(get_delimeter(r#"	"#).unwrap(), '\t');
        assert_eq!(get_delimeter(" ").unwrap(), ' ');
        assert_eq!(get_delimeter(",").unwrap(), ',');
        assert!(get_delimeter("::").is_err());
    }

    #[test]
    fn test_select_columns() {
        let file_rdr = open_reader("testdata/small.1.regenie").unwrap();
        let mut wtr = Cursor::new(Vec::new());
        process_file(file_rdr, &mut wtr, vec!["CHROM".into(), "ID".into(), "LOG10P".into()], ' ').unwrap();
        let desired_result = fs::read_to_string("testdata/small.1.CHR.ID.LOG10P.regenie").unwrap();
        let result = String::from_utf8(wtr.into_inner()).unwrap();
        assert_eq!(result, desired_result);
    }
}