use clap::Parser;
use std::{collections::HashMap, io};

use gwas_utils::{GuError, Result, get_delimeter_from_cli_argument, open_reader, open_writer};

use crate::csv::lib::{column_not_found_error, get_csv_reader, get_csv_writer};

pub(crate) const ABOUT: &str = "Reheader a CSV file";
pub(crate) const USAGE: &str = r#"
    gu csv reheader infile.csv[.gz] -l COL1 COL2 ... [-o outfile.csv[.gz]]
    gu csv reheader infile.csv[.gz] -c OLDCOL1=NEWCOL1 OLDCOL2=NEWCOL2 ... [-o outfile.csv[.gz]]"#;

pub(crate) fn get_usage() -> String {
    USAGE.to_string()
}

#[derive(Parser, Debug)]
pub(crate) struct Args {
    /// CSV file to process (can be gzipped if filename ends with .gz)
    #[arg(default_value = "stdin")]
    input: String,

    /// New header to write (format: list of strings corresponding to new columm names)
    #[arg(short, long, num_args = 1.., conflicts_with = "columns")]
    list: Vec<String>,

    /// Specific columns to rename (format: oldcolumn=newcolumn for arbitrary number of columns)
    #[arg(short, long, num_args = 1.., conflicts_with = "list")]
    columns: Vec<String>,

    /// Delimiter for CSV file reading and writing
    #[arg(short, long, default_value = "auto")]
    delim: String,

    /// CSV file to write (will be gzipped if filename ends with .gz)
    #[arg(short, long, default_value = "stdout")]
    output: String,
}

pub(crate) fn run(args: Args) -> Result<()> {
    let (file_rdr, file_wtr, newheader, newcolumns, sep) = handle_commandline_args(args)?;
    process_file(file_rdr, file_wtr, newheader, newcolumns, sep)?;
    Ok(())
}

fn handle_commandline_args(
    args: Args,
) -> Result<(
    gwas_utils::Reader,
    gwas_utils::Writer,
    Option<csv::StringRecord>,
    Option<HashMap<String, String>>,
    char,
)> {
    let mut file_rdr = open_reader(&args.input)?;
    let file_wtr = open_writer(&args.output)?;
    let sep = match args.delim.as_str() {
        "auto" => file_rdr.sniff_csv_delimiter()?,
        _ => get_delimeter_from_cli_argument(&args.delim)?,
    };
    let new_header = if !args.list.is_empty() {
        let mut sr = csv::StringRecord::new();
        for h in args.list {
            sr.push_field(&h);
        }
        Some(sr)
    } else {
        None
    };
    let new_columns = if !args.columns.is_empty() {
        let mut m: HashMap<String, String> = HashMap::new();
        for val in args.columns {
            let parts: Vec<&str> = val.split('=').collect();
            if parts.len() == 2 {
                m.insert(parts[0].to_string(), parts[1].to_string());
            } else {
                return Err(GuError::Message(format!(
                    "Invalid column rename argument: {}. Must be in the format oldcolumn=newcolumn",
                    val
                )));
            }
        }
        Some(m)
    } else {
        None
    };

    Ok((file_rdr, file_wtr, new_header, new_columns, sep))
}

fn process_file<R, W>(
    rdr: R,
    wtr: W,
    newheader: Option<csv::StringRecord>,
    newcolumns: Option<HashMap<String, String>>,
    sep: char,
) -> Result<()>
where
    R: io::Read,
    W: io::Write,
{
    let mut csv_rdr = get_csv_reader(rdr, sep);

    let mut csv_wtr = get_csv_writer(wtr, sep);
    let header = csv_rdr.headers()?.clone();

    if let Some(newheader) = newheader {
        if newheader.len() != header.len() {
            return Err(GuError::Message(format!(
                "New header has {} columns, but input file has {} columns",
                newheader.len(),
                header.len()
            )));
        }
        csv_wtr.write_record(&newheader)?;
    } else if let Some(newcolumns) = newcolumns {
        let mut new_header: Vec<String> = header.iter().map(|h| h.to_string()).collect();
        for (old_col, new_col) in newcolumns {
            if let Some(idx) = header.iter().position(|h| h == old_col) {
                new_header[idx] = new_col;
            } else {
                return Err(column_not_found_error(&old_col));
            }
        }
        csv_wtr.write_record(&new_header)?;
    } else {
        // No reheadering requested, just write the original header
        csv_wtr.write_record(&header)?;
    }

    for result in csv_rdr.records() {
        let record = result?;
        csv_wtr.write_record(&record)?;
    }

    csv_wtr.flush()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    static INPUT: &str = r#"CHROM GENPOS ID ALLELE0 ALLELE1 A1FREQ INFO N TEST BETA SE CHISQ LOG10P EXTRA
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

    static DESIRED_RESULT: &str = r#"CHR POS ID ALLELE0 ALLELE1 A1FREQ INFO N TEST BETA SE CHISQ LOG10P EXTRA
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

    #[test]
    fn test_reheader_l() {
        let mut wtr = Cursor::new(Vec::new());

        process_file(
            std::io::Cursor::new(INPUT.as_bytes()),
            &mut wtr,
            Some(csv::StringRecord::from(vec![
                "CHR", "POS", "ID", "ALLELE0", "ALLELE1", "A1FREQ", "INFO", "N", "TEST", "BETA",
                "SE", "CHISQ", "LOG10P", "EXTRA",
            ])),
            None,
            ' ',
        )
        .unwrap();

        let result = String::from_utf8(wtr.into_inner()).unwrap();

        assert_eq!(result, DESIRED_RESULT);
    }

    #[test]
    fn test_reheader_c() {
        let mut wtr = Cursor::new(Vec::new());

        process_file(
            std::io::Cursor::new(INPUT.as_bytes()),
            &mut wtr,
            None,
            Some(
                vec![
                    ("CHROM".to_string(), "CHR".to_string()),
                    ("GENPOS".to_string(), "POS".to_string()),
                ]
                .into_iter()
                .collect(),
            ),
            ' ',
        )
        .unwrap();

        let result = String::from_utf8(wtr.into_inner()).unwrap();

        assert_eq!(result, DESIRED_RESULT);
    }
}
