use clap::Parser;
use std::collections::HashMap;
use std::io;

use gwas_utils::{GuError, Result, get_delimeter_from_cli_argument, open_reader, open_writer};

use crate::csv::err;

pub(crate) const ABOUT: &str =
    "Split a CSV file into multiple files based on unique values in a specified categorical column";
pub(crate) const USAGE: &str = "gu csv split infile.csv[.gz] -c colname";

pub(crate) fn get_usage() -> String {
    USAGE.to_string()
}

#[derive(Parser, Debug)]
pub(crate) struct Args {
    /// CSV file to process (can be gzipped if filename ends with .gz)
    #[arg(default_value = "stdin")]
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
        "auto" => file_rdr.sniff_csv_delimiter()?,
        _ => get_delimeter_from_cli_argument(&args.delim)?,
    };
    let suffix = match args.suffix.is_empty() {
        true => "".to_string(),
        false => format!(".{}", args.suffix),
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
    let column_index = header
        .iter()
        .position(|h| h == column_to_split_on)
        .ok_or(err::column_not_found_error(&column_to_split_on))?;

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
        let input = r#"CHROM GENPOS ID ALLELE0 ALLELE1 A1FREQ INFO N TEST BETA SE CHISQ LOG10P EXTRA
1 1 1 2 1 0.214575 1 494 ADD 0.0775674 0.230001 0.113736 0.133163 NA
1 2 2 2 1 0.218623 1 494 ADD 0.131068 0.239808 0.29872 0.233077 NA
1 3 3 2 1 0.211538 1 494 ADD -0.256723 0.244611 1.10148 0.531739 NA
1 4 4 2 1 0.191296 1 494 ADD -0.131175 0.250523 0.274164 0.221449 NA
1 5 5 2 1 0.195344 1 494 ADD -0.187228 0.235372 0.632751 0.370236 NA
2 1 5 2 1 0.195344 1 494 ADD -0.187228 0.235372 0.632751 0.370236 NA
2 2 6 2 1 0.190283 1 494 ADD -0.234935 0.245557 0.91536 0.47019 NA
2 3 7 2 1 0.206478 1 494 ADD 0.11647 0.227747 0.26153 0.215332 NA
2 4 8 2 1 0.188259 1 494 ADD -0.353772 0.251712 1.97533 0.796197 NA
2 5 9 2 1 0.194332 1 494 ADD 0.283254 0.241072 1.38057 0.619781 NA
"#;

        process_file(
            std::io::Cursor::new(input.as_bytes()),
            "CHROM".to_string(),
            ' ',
            ".csv".to_string(),
        )
        .unwrap();

        let desired_result_1 =
            r#"CHROM GENPOS ID ALLELE0 ALLELE1 A1FREQ INFO N TEST BETA SE CHISQ LOG10P EXTRA
1 1 1 2 1 0.214575 1 494 ADD 0.0775674 0.230001 0.113736 0.133163 NA
1 2 2 2 1 0.218623 1 494 ADD 0.131068 0.239808 0.29872 0.233077 NA
1 3 3 2 1 0.211538 1 494 ADD -0.256723 0.244611 1.10148 0.531739 NA
1 4 4 2 1 0.191296 1 494 ADD -0.131175 0.250523 0.274164 0.221449 NA
1 5 5 2 1 0.195344 1 494 ADD -0.187228 0.235372 0.632751 0.370236 NA
"#
            .to_string();

        let desired_result_2 =
            r#"CHROM GENPOS ID ALLELE0 ALLELE1 A1FREQ INFO N TEST BETA SE CHISQ LOG10P EXTRA
2 1 5 2 1 0.195344 1 494 ADD -0.187228 0.235372 0.632751 0.370236 NA
2 2 6 2 1 0.190283 1 494 ADD -0.234935 0.245557 0.91536 0.47019 NA
2 3 7 2 1 0.206478 1 494 ADD 0.11647 0.227747 0.26153 0.215332 NA
2 4 8 2 1 0.188259 1 494 ADD -0.353772 0.251712 1.97533 0.796197 NA
2 5 9 2 1 0.194332 1 494 ADD 0.283254 0.241072 1.38057 0.619781 NA
"#
            .to_string();

        let result_1 = fs::read_to_string("CHROM.1.csv").unwrap();
        let result_2 = fs::read_to_string("CHROM.2.csv").unwrap();
        assert_eq!(result_1, desired_result_1);
        assert_eq!(result_2, desired_result_2);
        fs::remove_file("CHROM.1.csv").unwrap();
        fs::remove_file("CHROM.2.csv").unwrap();
    }
}
