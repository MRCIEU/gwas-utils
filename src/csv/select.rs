use clap::Parser;
use std::io;

use gwas_utils::{GuError, Result, get_delimeter_from_cli_argument, open_reader, open_writer};

pub(crate) const USAGE: &str =
    "gu csv select infile.csv[.gz] -c <column1 column2 ...> [-o outfile.csv[.gz]]";
pub(crate) const ABOUT: &str = "Select specific columns from a CSV file";

#[derive(Parser, Debug)]
pub(crate) struct Args {
    /// CSV file to process (can be gzipped if filename ends with .gz)
    #[arg(default_value = "stdin")]
    input: String,

    /// Column names to select
    #[arg(short, long, num_args = 1..)]
    columns: Vec<String>,

    /// Delimiter for CSV file reading and writing
    #[arg(short, long, default_value = "auto")]
    delim: String,

    /// CSV file to write (will be gzipped if filename ends with .gz)
    #[arg(short, long, default_value = "stdout")]
    output: String,
}

pub(crate) fn run(args: Args) -> Result<()> {
    let (file_rdr, file_wtr, columns_to_select, sep) = handle_commandline_args(args)?;
    process_file(file_rdr, file_wtr, columns_to_select, sep)
}

fn handle_commandline_args(
    args: Args,
) -> Result<(gwas_utils::Reader, gwas_utils::Writer, Vec<String>, char)> {
    let mut file_rdr = open_reader(&args.input)?;
    let file_wtr = open_writer(&args.output)?;
    let sep = match args.delim.as_str() {
        "auto" => file_rdr.sniff()?,
        _ => get_delimeter_from_cli_argument(&args.delim)?,
    };
    Ok((file_rdr, file_wtr, args.columns, sep))
}

fn process_file<R, W>(rdr: R, wtr: W, columns_to_select: Vec<String>, sep: char) -> Result<()>
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

    let header_reduced = column_indices_to_retain
        .iter()
        .map(|&i| &header[i])
        .collect::<Vec<_>>();
    if header_reduced.is_empty() {
        return Err(GuError::Message("No matching columns found".into()));
    }

    let mut csv_wtr = csv::WriterBuilder::new()
        .delimiter(sep as u8)
        .from_writer(wtr);

    csv_wtr.write_record(&header_reduced)?;

    for result in csv_rdr.records() {
        let record = result?;
        let record_reduced = column_indices_to_retain
            .iter()
            .map(|&i| &record[i])
            .collect::<Vec<_>>();
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
    fn test_select_columns() {
        let file_rdr = open_reader("testdata/small.1.regenie").unwrap();
        let mut wtr = Cursor::new(Vec::new());
        process_file(
            file_rdr,
            &mut wtr,
            vec!["CHROM".into(), "ID".into(), "LOG10P".into()],
            ' ',
        )
        .unwrap();
        let desired_result = fs::read_to_string("testdata/small.1.CHR.ID.LOG10P.regenie").unwrap();
        let result = String::from_utf8(wtr.into_inner()).unwrap();
        assert_eq!(result, desired_result);
    }
}
