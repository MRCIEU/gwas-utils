use clap::Parser;
use std::error::Error;
use std::io;
use std::process;

use gwas_utilities::{open_reader, open_writer};

const USAGE: &str = "regenie_add_pval_col -i infile.regenie[.gz] -o outfile.regenie[.gz]";

#[derive(Parser, Debug)]
#[command(version, override_usage = USAGE, about = "Add a P column to a Regenie output file based on the LOG10P column. The P column is added as the last column in the file. If the LOG10P value is large enough that the corresponding P value would be smaller than the smallest positive normal number representable in f64, then the P value is set to that smallest positive normal number (f64::MIN_POSITIVE) to avoid underflow issues when converting back and forth between log10(P) and P.")]
struct Args {
    /// Regenie output file to process (can be gzipped if filename ends with .gz)
    #[arg(short, long)]
    input: String,

    /// Output file to write with added p-value column (will be gzipped if filename ends with .gz)
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
    let (file_rdr, file_wtr) = handle_commandline_args()?;
    process_file(file_rdr, file_wtr)
}

fn handle_commandline_args()
-> Result<(gwas_utilities::Reader, gwas_utilities::Writer), Box<dyn Error>> {
    let args = Args::parse();
    let file_rdr = open_reader(&args.input)?;
    let file_wtr = open_writer(&args.output)?;
    Ok((file_rdr, file_wtr))
}

fn process_file<R, W>(rdr: R, wtr: W) -> Result<(), Box<dyn Error>>
where
    R: io::Read,
    W: io::Write,
{
    let mut csv_rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .delimiter(b' ')
        .from_reader(rdr);

    let mut header = csv_rdr.headers()?.clone();
    let (mut log10_p_col_idx, mut log10_p_col_found) = (0, false);
    for (i, val) in header.iter().enumerate() {
        if val == "LOG10P" {
            log10_p_col_idx = i;
            log10_p_col_found = true;
            break;
        }
    }
    if !log10_p_col_found {
        return Err("couldn't find LOG10P column in file header".into());
    }

    header.push_field("P");

    let mut csv_wtr = csv::WriterBuilder::new().delimiter(b' ').from_writer(wtr);

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
        process_file(file_rdr, &mut wtr).unwrap();
        let desired_result = fs::read_to_string("testdata/small.concat.P.regenie").unwrap();
        let result = String::from_utf8(wtr.into_inner()).unwrap();
        assert_eq!(result, desired_result);
    }
}
