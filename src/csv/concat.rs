use clap::Parser;
use csv::StringRecord;
use std::io;

use gwas_utils::{GuError, Result, get_delimeter_from_cli_argument, open_reader, open_writer};

use crate::csv::lib;

pub(crate) const ABOUT: &str = "Concatenate multiple CSV files into a single file";
pub(crate) const USAGE: &str =
    "gu csv concat infile1.csv[.gz] infile2.csv[.gz] ... [-o outfile.csv[.gz]]";

pub(crate) fn get_usage() -> String {
    USAGE.to_string()
}

#[derive(Parser, Debug)]
pub(crate) struct Args {
    /// CSV files to concatenate (can be gzipped if filenames end with .gz)
    #[arg(num_args = 1.., required = true)]
    input: Vec<String>,

    /// Delimiter for CSV file reading and writing
    #[arg(short, long, default_value = "auto")]
    delim: String,

    /// CSV file to write (will be gzipped if filename ends with .gz)
    #[arg(short, long, default_value = "stdout")]
    output: String,
}

pub(crate) fn run(args: Args) -> Result<()> {
    let (readers, file_wtr, sep) = handle_commandline_args(args)?;
    process_files(readers, file_wtr, sep)?;
    Ok(())
}

fn handle_commandline_args(
    args: Args,
) -> Result<(Vec<gwas_utils::Reader>, gwas_utils::Writer, char)> {
    let mut readers: Vec<gwas_utils::Reader> = Vec::new();
    for filename in args.input.iter() {
        let rdr = open_reader(filename)?;
        readers.push(rdr);
    }

    let file_wtr: gwas_utils::Writer = open_writer(&args.output)?;

    let sep = match args.delim.as_str() {
        "auto" => {
            let mut seps: Vec<char> = Vec::new();
            for rdr in readers.iter_mut() {
                seps.push(rdr.sniff_csv_delimiter()?);
            }
            if seps.iter().all(|&s| s == seps[0]) {
                seps[0]
            } else {
                return Err(GuError::Message(
                    "Inconsistent delimiters across input files".into(),
                ));
            }
        }
        _ => get_delimeter_from_cli_argument(&args.delim)?,
    };
    Ok((readers, file_wtr, sep))
}

fn process_files<R, W>(rdrs: Vec<R>, wtr: W, sep: char) -> Result<()>
where
    R: io::Read,
    W: io::Write,
{
    let mut csv_wtr = lib::get_csv_writer(wtr, sep);
    let mut index_header = StringRecord::new();

    for (i, rdr) in rdrs.into_iter().enumerate() {
        let mut csv_rdr = lib::get_csv_reader(rdr, sep);
        if i == 0 {
            index_header = csv_rdr.headers()?.clone();
            csv_wtr.write_record(&index_header)?;
        } else {
            let header = csv_rdr.headers()?.clone();
            if header != index_header {
                return Err(GuError::Message("Mismatched headers in input files".into()));
            }
        }
        for result in csv_rdr.records() {
            let record = result?;
            csv_wtr.write_record(&record)?;
        }
    }

    csv_wtr.flush()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn test_concat_files() {
        let input1 = r#"CHROM GENPOS ID ALLELE0 ALLELE1 A1FREQ INFO N TEST BETA SE CHISQ LOG10P EXTRA
1 1 1 2 1 0.214575 1 494 ADD 0.0775674 0.230001 0.113736 0.133163 NA
1 2 2 2 1 0.218623 1 494 ADD 0.131068 0.239808 0.29872 0.233077 NA
1 3 3 2 1 0.211538 1 494 ADD -0.256723 0.244611 1.10148 0.531739 NA
1 4 4 2 1 0.191296 1 494 ADD -0.131175 0.250523 0.274164 0.221449 NA
1 5 5 2 1 0.195344 1 494 ADD -0.187228 0.235372 0.632751 0.370236 NA
"#;

        let input2 = r#"CHROM GENPOS ID ALLELE0 ALLELE1 A1FREQ INFO N TEST BETA SE CHISQ LOG10P EXTRA
2 1 5 2 1 0.195344 1 494 ADD -0.187228 0.235372 0.632751 0.370236 NA
2 2 6 2 1 0.190283 1 494 ADD -0.234935 0.245557 0.91536 0.47019 NA
2 3 7 2 1 0.206478 1 494 ADD 0.11647 0.227747 0.26153 0.215332 NA
2 4 8 2 1 0.188259 1 494 ADD -0.353772 0.251712 1.97533 0.796197 NA
2 5 9 2 1 0.194332 1 494 ADD 0.283254 0.241072 1.38057 0.619781 NA
"#;

        let readers = vec![
            (Cursor::new(input1.as_bytes())),
            (Cursor::new(input2.as_bytes())),
        ];

        let mut wtr = Cursor::new(Vec::new());
        process_files(readers, &mut wtr, ' ').unwrap();

        let desired_result = r#"CHROM GENPOS ID ALLELE0 ALLELE1 A1FREQ INFO N TEST BETA SE CHISQ LOG10P EXTRA
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

        let result = String::from_utf8(wtr.into_inner()).unwrap();
        assert_eq!(result, desired_result);
    }
}
