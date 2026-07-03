use clap::Parser;
use std::io;

use gwas_utils::{Result, get_delimeter_from_cli_argument, open_reader, open_writer};

use crate::csv::err;

pub(crate) const ABOUT: &str = "Merge two CSV files based on a shared column";
pub(crate) const USAGE: &str =
    "gu csv merge infile1.csv[.gz] infile2.csv[.gz] -c KEY_COLUMN [-o outfile.csv[.gz]]";

pub(crate) fn get_usage() -> String {
    USAGE.to_string()
}

#[derive(Parser, Debug)]
pub(crate) struct Args {
    /// CSV files to merge (can be gzipped if filenames end with .gz)
    #[arg(num_args = 2, required = true)]
    input: Vec<String>,

    /// Column name to merge on
    #[arg(short, long, num_args = 1, required = true)]
    column: String,

    /// Column name to merge on for second file, if different
    #[arg(long)]
    c2: String,

    /// Delimiter for CSV file reading and writing
    #[arg(long, default_value = "auto")]
    d1: String,

    /// Delimiter for the second CSV file
    #[arg(long, default_value = "auto")]
    d2: String,

    /// CSV file to write (will be gzipped if filename ends with .gz)
    #[arg(short, long, default_value = "stdout")]
    output: String,
}

pub(crate) fn run(args: Args) -> Result<()> {
    let (file_rdr1, file_rdr2, file_wtr, merge_column1, merge_column2, sep1, sep2) =
        handle_commandline_args(args)?;
    process_files(
        file_rdr1,
        file_rdr2,
        file_wtr,
        merge_column1,
        merge_column2,
        sep1,
        sep2,
    )
}

fn handle_commandline_args(
    args: Args,
) -> Result<(
    gwas_utils::Reader,
    gwas_utils::Reader,
    gwas_utils::Writer,
    String,
    String,
    char,
    char,
)> {
    let mut file_rdr1 = open_reader(&args.input[0])?;
    let mut file_rdr2 = open_reader(&args.input[1])?;
    let file_wtr = open_writer(&args.output)?;
    let sep1 = match args.d1.as_str() {
        "auto" => file_rdr1.sniff_csv_delimiter()?,
        _ => get_delimeter_from_cli_argument(&args.d1)?,
    };
    let sep2 = match args.d2.as_str() {
        "auto" => file_rdr2.sniff_csv_delimiter()?,
        _ => get_delimeter_from_cli_argument(&args.d2)?,
    };
    Ok((
        file_rdr1,
        file_rdr2,
        file_wtr,
        args.column,
        args.c2,
        sep1,
        sep2,
    ))
}

fn process_files<R, W>(
    rdr1: R,
    rdr2: R,
    wtr: W,
    merge_column1: String,
    mut merge_column2: String,
    sep1: char,
    sep2: char,
) -> Result<()>
where
    R: io::Read,
    W: io::Write,
{
    let (header1, loaded) = load_csv(rdr1, sep1, merge_column1.clone())?;

    let mut csv_rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .delimiter(sep2 as u8)
        .from_reader(rdr2);

    let header2 = csv_rdr.headers()?.clone();

    if merge_column2.is_empty() {
        merge_column2 = merge_column1.clone();
    }

    let key_column_idx = header2
        .iter()
        .position(|h| h == merge_column2)
        .ok_or(err::column_not_found_error(&merge_column2))?;

    let combined_header = combine_records(header1, header2, key_column_idx)?;

    let mut csv_wtr = csv::WriterBuilder::new()
        .delimiter(sep1 as u8)
        .from_writer(wtr);

    csv_wtr.write_record(&combined_header)?;

    for result in csv_rdr.records() {
        let record = result?;
        let key_value = record
            .get(key_column_idx)
            .ok_or(err::column_not_found_error(&merge_column2))?
            .to_string();
        if let Some(matching_record) = loaded.get(&key_value) {
            let combined_record = combine_records(matching_record.clone(), record, key_column_idx)?;
            csv_wtr.write_record(&combined_record)?;
        }
    }

    Ok(())
}

fn load_csv<R>(
    rdr: R,
    sep: char,
    key: String,
) -> Result<(
    csv::StringRecord,
    std::collections::HashMap<String, csv::StringRecord>,
)>
where
    R: io::Read,
{
    let mut csv_rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .delimiter(sep as u8)
        .from_reader(rdr);

    let header = csv_rdr.headers()?.clone();

    // Set column indices for filters based on header
    let key_column_idx = header
        .iter()
        .position(|h| h == key)
        .ok_or(err::column_not_found_error(&key))?;

    let mut map = std::collections::HashMap::new();

    for result in csv_rdr.records() {
        let record = result?;
        let key_value = record
            .get(key_column_idx)
            .ok_or(err::column_not_found_error(&key))?
            .to_string();
        map.insert(key_value, record.clone());
    }

    Ok((header, map))
}

fn combine_records(
    r1: csv::StringRecord,
    r2: csv::StringRecord,
    key_column_idx: usize,
) -> Result<csv::StringRecord> {
    let mut combined = r1.clone();
    for (i, field) in r2.iter().enumerate() {
        if i != key_column_idx {
            combined.push_field(field);
        }
    }
    Ok(combined)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test1() {
        let input1 = r#"ID,VALUE1
A,1
B,2
C,3
D,4
E,5
"#;

        let input2 = r#"ID,VALUE2
A,101
B,102
C,103
D,104
E,105
"#;

        let mut wtr = std::io::Cursor::new(Vec::new());

        process_files(
            std::io::Cursor::new(input1.as_bytes()),
            std::io::Cursor::new(input2.as_bytes()),
            &mut wtr,
            "ID".to_string(),
            "ID".to_string(),
            ',',
            ',',
        )
        .unwrap();

        let desired_result_str = r#"ID,VALUE1,VALUE2
A,1,101
B,2,102
C,3,103
D,4,104
E,5,105
"#;

        let result_str = String::from_utf8(wtr.into_inner()).unwrap();

        assert_eq!(result_str, desired_result_str);
    }

    #[test]
    fn test2() {
        let input1 = r#"ID,VALUE1
A,1
B,2
C,3
D,4
E,5
"#;

        let input2 = r#"EID,VALUE2
A,101
B,102
C,103
D,104
E,105
"#;

        let mut wtr = std::io::Cursor::new(Vec::new());

        process_files(
            std::io::Cursor::new(input1.as_bytes()),
            std::io::Cursor::new(input2.as_bytes()),
            &mut wtr,
            "ID".to_string(),
            "EID".to_string(),
            ',',
            ',',
        )
        .unwrap();

        let desired_result_str = r#"ID,VALUE1,VALUE2
A,1,101
B,2,102
C,3,103
D,4,104
E,5,105
"#;

        let result_str = String::from_utf8(wtr.into_inner()).unwrap();

        assert_eq!(result_str, desired_result_str);
    }
}
