use clap::Parser;
use std::io;

use gwas_utils::{Result, get_delimeter_from_cli_argument, open_reader, open_writer};

use crate::csv::lib::{
    get_column_idx_from_name, get_column_value_from_idx, get_csv_reader, get_csv_writer,
};

pub(crate) const ABOUT: &str =
    "Add a Z-score column to a CSV file based on a beta column and a standard error column";
pub(crate) const USAGE: &str = "gu csv addz infile.regenie[.gz] [-o outfile.regenie[.gz]]";

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

    /// Name of column containing [optionally negative] BETA values
    #[arg(long, default_value = "BETA")]
    beta: String,

    /// Name of column containing standard error values
    #[arg(long, default_value = "SE")]
    se: String,

    /// CSV file to write (will be gzipped if filename ends with .gz)
    #[arg(short, long, default_value = "stdout")]
    output: String,
}

pub(crate) fn run(args: Args) -> Result<()> {
    let (file_rdr, file_wtr, sep, beta_col_name, se_col_name) = handle_commandline_args(args)?;
    process_file(file_rdr, file_wtr, sep, beta_col_name, se_col_name)
}

fn handle_commandline_args(
    args: Args,
) -> Result<(gwas_utils::Reader, gwas_utils::Writer, char, String, String)> {
    let mut file_rdr = open_reader(&args.input)?;
    let file_wtr = open_writer(&args.output)?;
    let sep = match args.delim.as_str() {
        "auto" => file_rdr.sniff_csv_delimiter()?,
        _ => get_delimeter_from_cli_argument(&args.delim)?,
    };
    Ok((file_rdr, file_wtr, sep, args.beta, args.se))
}

fn process_file<R, W>(rdr: R, wtr: W, sep: char, beta_col: String, se_col: String) -> Result<()>
where
    R: io::Read,
    W: io::Write,
{
    let mut csv_rdr = get_csv_reader(rdr, sep);
    let mut header = csv_rdr.headers()?.clone();
    let beta_col_idx = get_column_idx_from_name(&header, &beta_col)?;
    let se_col_idx = get_column_idx_from_name(&header, &se_col)?;
    header.push_field("Z");

    let mut csv_wtr = get_csv_writer(wtr, sep);
    csv_wtr.write_record(&header)?;

    for result in csv_rdr.records() {
        let mut record = result?;
        let beta_val = get_column_value_from_idx(&record, beta_col_idx)?;
        let se_val = get_column_value_from_idx(&record, se_col_idx)?;
        let z_val = get_z_from_beta_se(beta_val, se_val)?;
        record.push_field(&z_val);
        csv_wtr.write_record(&record)?;
    }

    csv_wtr.flush()?;

    Ok(())
}

fn get_z_from_beta_se(beta_val: &str, se_val: &str) -> Result<String> {
    let result = beta_val
        .parse::<f64>()
        .ok()
        .zip(se_val.parse::<f64>().ok())
        .filter(|(_, se)| *se != 0.0)
        .map(|(beta, se)| format!("{:.6}", beta / se))
        .unwrap_or_else(|| "NA".into());
    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn test_get_z_value() {
        let beta = "0.1";
        let se = "0.2";
        let z = get_z_from_beta_se(beta, se).unwrap();
        assert_eq!(z, format!("{:.6}", 0.5));
    }

    #[test]
    fn test_add_z_to_file() {
        let input = r#"BETA	SE
0.0178576	0.0227561
-0.155803	0.00137283
13.2827	0.0216606
-0.00589905	0.000411222
0.0288565	0.00123684
-0.0206294	0.00218886
0.00538613	0.00160487
-0.0853803	0.00153331
-0.00465922	0.00552664
-0.0423111	0.00336009
"#;

        let mut wtr = Cursor::new(Vec::new());

        process_file(
            std::io::Cursor::new(input.as_bytes()),
            &mut wtr,
            '\t',
            "BETA".to_string(),
            "SE".to_string(),
        )
        .unwrap();

        let result = String::from_utf8(wtr.into_inner()).unwrap();

        let desired_result = r#"BETA	SE	Z
0.0178576	0.0227561	0.784739
-0.155803	0.00137283	-113.490381
13.2827	0.0216606	613.219394
-0.00589905	0.000411222	-14.345171
0.0288565	0.00123684	23.330827
-0.0206294	0.00218886	-9.424723
0.00538613	0.00160487	3.356116
-0.0853803	0.00153331	-55.683652
-0.00465922	0.00552664	-0.843047
-0.0423111	0.00336009	-12.592252
"#;

        assert_eq!(result, desired_result);
    }
}
