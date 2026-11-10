use clap::Parser;
use std::io;

use gwas_utils::{GuError, Result, open_writer};

use crate::csv::lib::{column_not_found_error, get_csv_reader, get_csv_writer, open_input};

pub(crate) const ABOUT: &str = "Reheader a CSV file";
pub(crate) const USAGE: &str = r#"
    gu csv reheader infile.csv[.gz] -l COL1 COL2 ... [-o outfile.csv[.gz]]
    gu csv reheader infile.csv[.gz] -c OLDCOL1=NEWCOL1 OLDCOL2=NEWCOL2 ... [-o outfile.csv[.gz]]"#;

pub(crate) fn get_usage() -> String {
    USAGE.to_string()
}

#[derive(Parser, Debug)]
pub(crate) struct Args {
    /// CSV file to process (gzipped input is detected automatically)
    #[arg(default_value = "stdin")]
    input: String,

    /// New header to write (format: list of strings corresponding to new column names)
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
    let (file_rdr, file_wtr, new_header, sep) = handle_commandline_args(args)?;
    process_file(file_rdr, file_wtr, new_header, sep)?;
    Ok(())
}

enum NewHeader {
    /// Replace the whole header
    List(csv::StringRecord),
    /// Rename the given columns, as (old name, new name) pairs
    Rename(Vec<(String, String)>),
}

type ParsedArgs = (gwas_utils::Reader, gwas_utils::Writer, NewHeader, char);

fn handle_commandline_args(args: Args) -> Result<ParsedArgs> {
    let (file_rdr, sep) = open_input(&args.input, &args.delim)?;
    let file_wtr = open_writer(&args.output)?;
    let new_header = parse_new_header(args.list, args.columns)?;
    Ok((file_rdr, file_wtr, new_header, sep))
}

fn parse_new_header(list: Vec<String>, columns: Vec<String>) -> Result<NewHeader> {
    if !list.is_empty() {
        Ok(NewHeader::List(csv::StringRecord::from(list)))
    } else if !columns.is_empty() {
        let mut renames: Vec<(String, String)> = Vec::new();
        for arg in columns {
            let (old, new) = parse_rename(&arg)?;
            if renames.iter().any(|(o, _)| *o == old) {
                return Err(GuError::Message(format!(
                    "Column \"{}\" is renamed more than once",
                    old
                )));
            }
            renames.push((old, new));
        }
        Ok(NewHeader::Rename(renames))
    } else {
        Err(GuError::Message(
            "Must specify either a new header (-l) or columns to rename (-c)".to_string(),
        ))
    }
}

/// Parse "oldcolumn=newcolumn". Whitespace around either name is ignored.
/// The new name may itself contain "=", but the old name can't.
fn parse_rename(arg: &str) -> Result<(String, String)> {
    let invalid = || {
        GuError::Message(format!(
            "Invalid column rename argument: {}. Must be in the format oldcolumn=newcolumn",
            arg
        ))
    };
    let (old, new) = arg.split_once('=').ok_or_else(invalid)?;
    let (old, new) = (old.trim(), new.trim());
    if old.is_empty() || new.is_empty() {
        return Err(invalid());
    }
    Ok((old.to_string(), new.to_string()))
}

fn process_file<R, W>(rdr: R, wtr: W, new_header: NewHeader, sep: char) -> Result<()>
where
    R: io::Read,
    W: io::Write,
{
    let mut csv_rdr = get_csv_reader(rdr, sep);

    let mut csv_wtr = get_csv_writer(wtr, sep);
    let header = csv_rdr.headers()?.clone();

    match new_header {
        NewHeader::List(new_header) => {
            if new_header.len() != header.len() {
                return Err(GuError::Message(format!(
                    "New header has {} columns, but input file has {} columns",
                    new_header.len(),
                    header.len()
                )));
            }
            csv_wtr.write_record(&new_header)?;
        }
        NewHeader::Rename(renames) => {
            let mut new_header: Vec<String> = header.iter().map(|h| h.to_string()).collect();
            for (old_col, new_col) in renames {
                match header.iter().position(|h| h == old_col) {
                    Some(idx) => new_header[idx] = new_col,
                    None => return Err(column_not_found_error(&old_col)),
                }
            }
            csv_wtr.write_record(&new_header)?;
        }
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

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Args::command().debug_assert();
    }

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
            NewHeader::List(csv::StringRecord::from(vec![
                "CHR", "POS", "ID", "ALLELE0", "ALLELE1", "A1FREQ", "INFO", "N", "TEST", "BETA",
                "SE", "CHISQ", "LOG10P", "EXTRA",
            ])),
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
            NewHeader::Rename(vec![
                ("CHROM".to_string(), "CHR".to_string()),
                ("GENPOS".to_string(), "POS".to_string()),
            ]),
            ' ',
        )
        .unwrap();

        let result = String::from_utf8(wtr.into_inner()).unwrap();

        assert_eq!(result, DESIRED_RESULT);
    }

    #[test]
    fn test_parse_rename_trims_whitespace() {
        assert_eq!(
            parse_rename("OLD=NEW").unwrap(),
            ("OLD".to_string(), "NEW".to_string())
        );
        assert_eq!(
            parse_rename("  OLD =  NEW ").unwrap(),
            ("OLD".to_string(), "NEW".to_string())
        );
        // Only the first "=" separates the names, so a new name can contain one
        assert_eq!(
            parse_rename("OLD=a=b").unwrap(),
            ("OLD".to_string(), "a=b".to_string())
        );
    }

    #[test]
    fn test_parse_rename_rejects_malformed_arguments() {
        for bad in ["OLD", "OLD:NEW", "=NEW", "OLD=", " = ", ""] {
            assert!(parse_rename(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn test_parse_new_header_requires_an_operation() {
        assert!(parse_new_header(vec![], vec![]).is_err());
        assert!(parse_new_header(vec!["A".into()], vec![]).is_ok());
        assert!(parse_new_header(vec![], vec!["A=B".into()]).is_ok());
    }

    #[test]
    fn test_parse_new_header_rejects_a_column_renamed_twice() {
        let err = parse_new_header(vec![], vec!["A=B".into(), "A=C".into()]).err();
        assert!(err.unwrap().to_string().contains("more than once"));
    }

    #[test]
    fn test_reheader_c_unknown_column_is_an_error() {
        let result = process_file(
            std::io::Cursor::new(INPUT.as_bytes()),
            Cursor::new(Vec::new()),
            NewHeader::Rename(vec![("NOPE".to_string(), "X".to_string())]),
            ' ',
        );
        assert!(result.is_err());
    }
}
