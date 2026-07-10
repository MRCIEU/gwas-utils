use clap::Parser;
use std::io;

use gwas_utils::{Result, get_delimeter_from_cli_argument, open_reader, open_writer};

use crate::csv::lib::{
    get_column_idx_from_name, get_column_value_from_idx, get_csv_reader, get_csv_writer,
};

pub(crate) const ABOUT: &str = "Add a P column to a CSV file based on a (minus) LOG10P column";
pub(crate) const USAGE: &str = "gu csv addp infile.regenie[.gz] [-o outfile.regenie[.gz]]";

pub(crate) fn get_usage() -> String {
    USAGE.to_string()
}

#[derive(Parser, Debug)]
pub(crate) struct Args {
    /// CSV file to process (can be gzipped if filename ends with .gz)
    #[arg(default_value = "stdin")]
    input: String,

    /// Delimiter for CSV file reading and writing
    #[arg(short, long, default_value = "auto")]
    delim: String,

    /// Name of column containing [optionally negative] LOG10P values
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
        "auto" => file_rdr.sniff_csv_delimiter()?,
        _ => get_delimeter_from_cli_argument(&args.delim)?,
    };
    Ok((file_rdr, file_wtr, sep, args.log10p))
}

fn process_file<R, W>(rdr: R, wtr: W, sep: char, log10_p_col: String) -> Result<()>
where
    R: io::Read,
    W: io::Write,
{
    let mut csv_rdr = get_csv_reader(rdr, sep);
    let mut header = csv_rdr.headers()?.clone();
    let log10_p_col_idx = get_column_idx_from_name(&header, &log10_p_col)?;
    header.push_field("P");

    let mut csv_wtr = get_csv_writer(wtr, sep);
    csv_wtr.write_record(&header)?;

    for result in csv_rdr.records() {
        let mut record = result?;
        let log10_p_val = get_column_value_from_idx(&record, log10_p_col_idx)?;
        let ps = get_p_from_log10_p(log10_p_val)?;
        record.push_field(&ps);
        csv_wtr.write_record(&record)?;
    }

    csv_wtr.flush()?;

    Ok(())
}

fn get_p_from_log10_p(log10_p_val: &str) -> Result<String> {
    match log10_p_val.parse::<f64>() {
        Ok(log10_p) => {
            let temp = log10_p.abs() * -1.0; // Convert to negative number regardless of input sign (these are p-values so they must be <= 1)
            let mut p = f64::powf(10.0, temp);
            if p == 0.0 {
                p = f64::MIN_POSITIVE;
            }
            Ok(format!("{:E}", p))
        }
        Err(_) => Ok("NA".into()),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn test_precision() {
        let log10_p = "1000.0";
        let p = get_p_from_log10_p(log10_p).unwrap();
        assert_eq!(p, format!("{:E}", f64::MIN_POSITIVE));
    }

    #[test]
    fn test_get_p_value() {
        let mut log10_p = "1.0";
        let p = get_p_from_log10_p(log10_p).unwrap();
        assert_eq!(p, format!("{:E}", 0.1));

        log10_p = "2.0";
        let p = get_p_from_log10_p(log10_p).unwrap();
        assert_eq!(p, format!("{:E}", 0.01));

        log10_p = "-2.0";
        let p = get_p_from_log10_p(log10_p).unwrap();
        assert_eq!(p, format!("{:E}", 0.01));
    }

    #[test]
    fn test_add_p_to_file() {
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

        let mut wtr = Cursor::new(Vec::new());

        process_file(
            std::io::Cursor::new(input.as_bytes()),
            &mut wtr,
            ' ',
            "LOG10P".to_string(),
        )
        .unwrap();

        let result = String::from_utf8(wtr.into_inner()).unwrap();

        let desired_result = r#"CHROM GENPOS ID ALLELE0 ALLELE1 A1FREQ INFO N TEST BETA SE CHISQ LOG10P EXTRA P
1 1 1 2 1 0.214575 1 494 ADD 0.0775674 0.230001 0.113736 0.133163 NA 7.359308350850203E-1
1 2 2 2 1 0.218623 1 494 ADD 0.131068 0.239808 0.29872 0.233077 NA 5.846864106077305E-1
1 3 3 2 1 0.211538 1 494 ADD -0.256723 0.244611 1.10148 0.531739 NA 2.939415635709693E-1
1 4 4 2 1 0.191296 1 494 ADD -0.131175 0.250523 0.274164 0.221449 NA 6.005525287551413E-1
1 5 5 2 1 0.195344 1 494 ADD -0.187228 0.235372 0.632751 0.370236 NA 4.263477741622134E-1
2 1 5 2 1 0.195344 1 494 ADD -0.187228 0.235372 0.632751 0.370236 NA 4.263477741622134E-1
2 2 6 2 1 0.190283 1 494 ADD -0.234935 0.245557 0.91536 0.47019 NA 3.386959472360824E-1
2 3 7 2 1 0.206478 1 494 ADD 0.11647 0.227747 0.26153 0.215332 NA 6.090711097900752E-1
2 4 8 2 1 0.188259 1 494 ADD -0.353772 0.251712 1.97533 0.796197 NA 1.5988326188144125E-1
2 5 9 2 1 0.194332 1 494 ADD 0.283254 0.241072 1.38057 0.619781 NA 2.4000428742690416E-1
"#;

        assert_eq!(result, desired_result);
    }
}
