use clap::Parser;
use statrs::distribution::{ContinuousCDF, Normal};
use std::io::{self, Write};

use gwas_utils::{GuError, Result, open_reader, open_writer};

use crate::csv::lib::{
    get_column_idx_from_name, get_column_value_from_idx, get_csv_reader,
    get_delimeter_from_cli_argument, get_p_from_log10_p,
};

pub(crate) const ABOUT: &str =
    "Calculate the genomic inflation factor (lambda) from a P or (minus) LOG10P column";
pub(crate) const USAGE: &str = "gu csv stats lambda infile.csv[.gz] [--p P_COLUMN | --log10p LOG10P_COLUMN] [-o outfile.txt[.gz]]";

pub(crate) fn get_usage() -> String {
    USAGE.to_string()
}

// Median of a chi-squared distribution with 1 degree of freedom
const CHISQ_1DF_MEDIAN: f64 = 0.454_936_423_119_572_8;

#[derive(Parser, Debug)]
pub(crate) struct Args {
    /// CSV file to process (can be gzipped if filename ends with .gz)
    #[arg(default_value = "stdin")]
    input: String,

    /// Delimiter for CSV file reading
    #[arg(short, long, default_value = "auto")]
    delim: String,

    /// Name of column containing P values
    #[arg(short, long, default_value = "P", conflicts_with = "log10p")]
    p: String,

    /// Name of column containing [optionally negative] LOG10P values (use instead of --p)
    #[arg(long)]
    log10p: Option<String>,

    /// File to write lambda to (will be gzipped if filename ends with .gz)
    #[arg(short, long, default_value = "stdout")]
    output: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PColumnType {
    P,
    Log10P,
}

pub(crate) fn run(args: Args) -> Result<()> {
    let (file_rdr, mut file_wtr, sep, p_col, p_col_type) = handle_commandline_args(args)?;
    let lambda = process_file(file_rdr, sep, p_col, p_col_type)?;
    writeln!(file_wtr, "{:.6}", lambda)?;
    file_wtr.flush()?;
    Ok(())
}

fn handle_commandline_args(
    args: Args,
) -> Result<(
    gwas_utils::Reader,
    gwas_utils::Writer,
    char,
    String,
    PColumnType,
)> {
    let mut file_rdr = open_reader(&args.input)?;
    let file_wtr = open_writer(&args.output)?;
    let sep = match args.delim.as_str() {
        "auto" => file_rdr.sniff_csv_delimiter()?,
        _ => get_delimeter_from_cli_argument(&args.delim)?,
    };
    let (p_col, p_col_type) = match args.log10p {
        Some(log10p_col) => (log10p_col, PColumnType::Log10P),
        None => (args.p, PColumnType::P),
    };
    Ok((file_rdr, file_wtr, sep, p_col, p_col_type))
}

fn process_file<R>(rdr: R, sep: char, p_col: String, p_col_type: PColumnType) -> Result<f64>
where
    R: io::Read,
{
    let mut csv_rdr = get_csv_reader(rdr, sep);
    let header = csv_rdr.headers()?.clone();
    let p_col_idx = get_column_idx_from_name(&header, &p_col)?;

    let mut chisqs = Vec::new();
    for result in csv_rdr.records() {
        let record = result?;
        let p_val = get_column_value_from_idx(&record, p_col_idx)?;
        if let Some(p) = get_p(p_val, p_col_type)? {
            chisqs.push(get_chisq_1df_from_p(p));
        }
    }

    if chisqs.is_empty() {
        return Err(GuError::Message(format!(
            "No valid values found in column \"{}\"",
            p_col
        )));
    }

    Ok(get_median(&mut chisqs) / CHISQ_1DF_MEDIAN)
}

// Missing/non-numeric values (e.g. "NA") are skipped
fn get_p(p_val: &str, p_col_type: PColumnType) -> Result<Option<f64>> {
    let val = match p_val.parse::<f64>() {
        Ok(val) if !val.is_nan() => val,
        _ => return Ok(None),
    };
    let p = match p_col_type {
        PColumnType::P => val,
        PColumnType::Log10P => get_p_from_log10_p(val),
    };
    if !(0.0..=1.0).contains(&p) {
        return Err(GuError::Message(format!(
            "P value out of range [0, 1]: {}",
            p_val
        )));
    }
    Ok(Some(p))
}

fn get_chisq_1df_from_p(p: f64) -> f64 {
    if p == 0.0 {
        return f64::INFINITY;
    }
    Normal::standard().inverse_cdf(p / 2.0).powi(2)
}

fn get_median(vals: &mut [f64]) -> f64 {
    let n = vals.len();
    let (lower, upper, _) = vals.select_nth_unstable_by(n / 2, f64::total_cmp);
    let upper = *upper;
    match n % 2 {
        1 => upper,
        _ => {
            let lower = lower.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            (lower + upper) / 2.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{} != {}", a, b);
    }

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Args::command().debug_assert();
    }

    #[test]
    fn test_cli_p_column_options() {
        let args = Args::try_parse_from(["lambda", "in.csv"]).unwrap();
        assert_eq!(args.p, "P");
        assert_eq!(args.log10p, None);
        assert!(Args::try_parse_from(["lambda", "in.csv", "--p", "P", "--log10p", "L"]).is_err());
        assert!(Args::try_parse_from(["lambda", "in.csv", "--p", "PVAL"]).is_ok());
        assert!(Args::try_parse_from(["lambda", "in.csv", "--log10p", "L"]).is_ok());
    }

    #[test]
    fn test_get_chisq_1df_from_p() {
        // Reference values from R's qnorm()
        assert_close(get_chisq_1df_from_p(1.0), 0.0);
        assert_close(get_chisq_1df_from_p(0.05), 1.959963984540054_f64.powi(2));
        assert_close(get_chisq_1df_from_p(2e-10), 6.361340902404056_f64.powi(2));
        assert_eq!(get_chisq_1df_from_p(0.0), f64::INFINITY);
    }

    #[test]
    fn test_chisq_1df_median() {
        assert_close(get_chisq_1df_from_p(0.5), CHISQ_1DF_MEDIAN);
    }

    #[test]
    fn test_get_median() {
        assert_eq!(get_median(&mut [3.0, 1.0, 2.0]), 2.0);
        assert_eq!(get_median(&mut [4.0, 1.0, 3.0, 2.0]), 2.5);
    }

    #[test]
    fn test_get_p() {
        assert_eq!(get_p("0.01", PColumnType::P).unwrap(), Some(0.01));
        assert_close(get_p("2", PColumnType::Log10P).unwrap().unwrap(), 0.01);
        assert_close(get_p("-2", PColumnType::Log10P).unwrap().unwrap(), 0.01);
        assert_eq!(get_p("NA", PColumnType::P).unwrap(), None);
        assert!(get_p("1.5", PColumnType::P).is_err());
    }

    #[test]
    fn test_lambda_from_p_and_log10p() {
        let input = r#"ID P LOG10P
1 0.5 0.301029995663981
2 0.1 1
3 0.9 0.0457574905606751
4 NA NA
5 0.01 2
6 0.7 0.154901959985743
"#;

        let lambda_p = process_file(
            std::io::Cursor::new(input.as_bytes()),
            ' ',
            "P".to_string(),
            PColumnType::P,
        )
        .unwrap();

        let lambda_log10p = process_file(
            std::io::Cursor::new(input.as_bytes()),
            ' ',
            "LOG10P".to_string(),
            PColumnType::Log10P,
        )
        .unwrap();

        // Median P is 0.5, so lambda should be exactly 1
        assert_close(lambda_p, 1.0);
        assert_close(lambda_log10p, 1.0);
    }
}
