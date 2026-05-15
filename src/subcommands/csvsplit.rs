use clap::Parser;
use std::collections::HashMap;
use std::io;

use gwas_utils::{GuError, Result, get_delimeter_from_cli_argument, open_reader, open_writer};

pub(crate) const USAGE: &str = "gu csvsplit -i infile.csv[.gz] -c colname";
pub(crate) const ABOUT: &str =
    "Split a CSV file into multiple files based on unique values in a specified categorical column";

#[derive(Parser, Debug)]
pub(crate) struct Args {
    /// CSV file to process (can be gzipped if filename ends with .gz)
    #[arg(short, long, default_value = "stdin")]
    input: String,

    /// Categorical column name to split on
    #[arg(short, long)]
    column: String,

    /// Delimiter for CSV file reading and writing
    #[arg(short, long, default_value = "auto")]
    delim: String,

    /// Suffix to add to output filenames (by default output files will be named "COLNAME.VAL.csv")
    #[arg(short, long, default_value = "csv")]
    suffix: String,
}

pub(crate) fn run(args: Args) -> Result<()> {
    let (file_rdr, column_to_split_on, sep, suffix) = handle_commandline_args(args)?;
    process_file(file_rdr, column_to_split_on, sep, suffix)?;
    Ok(())
}

fn handle_commandline_args(args: Args) -> Result<(gwas_utils::Reader, String, char, String)> {
    let mut file_rdr = open_reader(&args.input)?;
    let sep = match args.delim.as_str() {
        "auto" => file_rdr.sniff()?,
        _ => get_delimeter_from_cli_argument(&args.delim)?,
    };
    let suffix = match args.suffix.is_empty() {
        true => "".to_string(),
        false => format!(".{}", args.suffix)
    };
    Ok((file_rdr, args.column, sep, suffix))
}

fn process_file<R>(rdr: R, column_to_split_on: String, sep: char, suffix: String) -> Result<()>
where
    R: io::Read,
{
    let mut csv_rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .delimiter(sep as u8)
        .from_reader(rdr);

    let header = csv_rdr.headers()?.clone();
    let column_index =
        header
            .iter()
            .position(|h| h == column_to_split_on)
            .ok_or(GuError::Message(format!(
                "Column '{}' not found in CSV headers",
                column_to_split_on
            )))?;

    let mut file_handles: HashMap<String, csv::Writer<gwas_utils::Writer>> = HashMap::new();
    for result in csv_rdr.records() {
        let record = result?;
        if let Some(value) = record.get(column_index) {
            if file_handles.contains_key(value) {
                let csv_wtr = file_handles.get_mut(value).ok_or(GuError::Message(format!(
                    "Failed to get file handle for value '{}'",
                    value
                )))?;
                csv_wtr.write_record(&record)?;
            } else {
                let file_handle = format!("{}.{}{}", column_to_split_on, value, suffix);
                let mut csv_wtr = csv::WriterBuilder::new()
                    .delimiter(sep as u8)
                    .from_writer(open_writer(&file_handle)?);
                csv_wtr.write_record(&header)?;
                csv_wtr.write_record(&record)?;
                file_handles.insert(value.to_string(), csv_wtr);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn test_split_file() {
        let file_rdr = open_reader("testdata/small.concat.regenie").unwrap();
        process_file(file_rdr, "CHROM".to_string(), ' ', ".csv".to_string()).unwrap();
        let desired_result_1 = fs::read_to_string("testdata/small.1.regenie").unwrap();
        let desired_result_2 = fs::read_to_string("testdata/small.2.regenie").unwrap();
        let result_1 = fs::read_to_string("CHROM.1.csv").unwrap();
        let result_2 = fs::read_to_string("CHROM.2.csv").unwrap();
        assert_eq!(result_1, desired_result_1);
        assert_eq!(result_2, desired_result_2);
        fs::remove_file("CHROM.1.csv").unwrap();
        fs::remove_file("CHROM.2.csv").unwrap();
    }
}
