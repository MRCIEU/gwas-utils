use clap::Parser;
use std::io;

use gwas_utils::{GuError, Result, get_delimeter_from_cli_argument, open_reader, open_writer};

pub(crate) const USAGE: &str = "gu csvaddp -i infile.regenie[.gz] -o outfile.regenie[.gz]";
pub(crate) const ABOUT: &str = "Add a P column to a CSV file based on a LOG10P column";

#[derive(Parser, Debug)]
pub(crate) struct Args {
    /// CSV file to process (can be gzipped if filename ends with .gz)
    #[arg(short, long, default_value = "stdin")]
    input: String,

    /// Delimiter for CSV file reading and writing
    #[arg(short, long, default_value = "auto")]
    delim: String,

    /// Name of column containing the LOG10P values
    #[arg(long, default_value = "LOG10P")]
    log10p: String,

    /// CSV file to write (will be gzipped if filename ends with .gz)
    #[arg(short, long, default_value = "stdout")]
    output: String,
}

pub(crate) fn run(args: Args) -> Result<()> {
    let (file_rdr, file_wtr, sep, log10p_col) = handle_commandline_args(args)?;
    process_file(file_rdr, file_wtr, sep, log10p_col)
}

fn handle_commandline_args(
    args: Args,
) -> Result<(gwas_utils::Reader, gwas_utils::Writer, char, String)> {
    let mut file_rdr = open_reader(&args.input)?;
    let file_wtr = open_writer(&args.output)?;
    let sep = match args.delim.as_str() {
        "auto" => file_rdr.sniff()?,
        _ => get_delimeter_from_cli_argument(&args.delim)?,
    };
    Ok((file_rdr, file_wtr, sep, args.log10p))
}

fn process_file<R, W>(rdr: R, wtr: W, sep: char, log10_p_col: String) -> Result<()>
where
    R: io::Read,
    W: io::Write,
{
    let mut csv_rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .delimiter(sep as u8)
        .from_reader(rdr);

    let mut header = csv_rdr.headers()?.clone();

    let log10_p_col_idx = header
        .iter()
        .position(|h| h == log10_p_col)
        .ok_or(GuError::Message(
            "Couldn't find LOG10P column in file header".into(),
        ))?;

    header.push_field("P");

    let mut csv_wtr = csv::WriterBuilder::new()
        .delimiter(sep as u8)
        .from_writer(wtr);

    csv_wtr.write_record(&header)?;

    for result in csv_rdr.records() {
        let mut record = result?;
        let log10_p: f64 = record[log10_p_col_idx].parse()?;
        let p = get_p_from_log10_p(log10_p);
        let ps = format!("{:E}", p);
        record.push_field(&ps);
        csv_wtr.write_record(&record)?;
    }

    csv_wtr.flush()?;

    Ok(())
}

fn get_p_from_log10_p(log10_p: f64) -> f64 {
    let mut p = f64::powf(10.0, -log10_p);
    if p == 0.0 {
        p = f64::MIN_POSITIVE;
    }
    p
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Cursor;

    use super::*;

    #[test]
    fn test_precision() {
        let log10_p = 1000.0;
        let p = get_p_from_log10_p(log10_p);
        assert_eq!(p, f64::MIN_POSITIVE);
    }

    #[test]
    fn test_get_p_value() {
        let mut log10_p = 1.0;
        let mut p = get_p_from_log10_p(log10_p);
        assert_eq!(p, 0.1);

        log10_p = 2.0;
        p = get_p_from_log10_p(log10_p);
        assert_eq!(p, 0.01);
    }

    #[test]
    fn test_add_p_to_file() {
        let file_rdr = open_reader("testdata/small.concat.regenie").unwrap();
        let mut wtr = Cursor::new(Vec::new());
        process_file(file_rdr, &mut wtr, ' ', "LOG10P".to_string()).unwrap();
        let desired_result = fs::read_to_string("testdata/small.concat.P.regenie").unwrap();
        let result = String::from_utf8(wtr.into_inner()).unwrap();
        assert_eq!(result, desired_result);
    }
}
