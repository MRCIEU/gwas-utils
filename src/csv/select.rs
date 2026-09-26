use clap::Parser;
use std::io;

use gwas_utils::{Result, open_reader, open_writer};

use crate::csv::lib::{
    column_idx_out_of_bounds_error, get_column_idx_from_name, get_column_indices_from_regexes,
    get_csv_reader, get_csv_writer, get_delimeter_from_cli_argument,
};

pub(crate) const ABOUT: &str = "Select specific columns from a CSV file";
pub(crate) const USAGE: &str = r#"
    gu csv select infile.csv[.gz] -c <column1 column2 ...> [-o outfile.csv[.gz]]
    gu csv select infile.csv[.gz] -i <index1 index2 ...> [-o outfile.csv[.gz]]
    gu csv select infile.csv[.gz] -r <REGEX1 REGEX2 ...> [-o outfile.csv[.gz]]"#;

pub(crate) fn get_usage() -> String {
    USAGE.to_string()
}

#[derive(Parser, Debug)]
pub(crate) struct Args {
    /// CSV file to process (can be gzipped if filename ends with .gz)
    #[arg(default_value = "stdin")]
    input: String,

    /// Column names to select or...
    #[arg(short, long, num_args = 1.., required = false)]
    columns: Vec<String>,

    /// Column indices (1-based) to select or...
    #[arg(short, long, num_args = 1.., required = false, conflicts_with = "columns")]
    indices: Vec<usize>,

    /// Regexes to select column names against
    #[arg(short, long, num_args = 1.., required = false, conflicts_with = "columns", conflicts_with = "indices")]
    regexes: Vec<String>,

    /// Delimiter for CSV file reading and writing
    #[arg(short, long, default_value = "auto")]
    delim: String,

    /// Don't reorder selected columns
    #[arg(long, default_value_t = false)]
    no_reorder: bool,

    /// CSV file to write (will be gzipped if filename ends with .gz)
    #[arg(short, long, default_value = "stdout")]
    output: String,
}

pub(crate) fn run(args: Args) -> Result<()> {
    let (file_rdr, file_wtr, columns_to_select, sep) = handle_commandline_args(&args)?;
    process_file(file_rdr, file_wtr, columns_to_select, args.no_reorder, sep)
}

fn handle_commandline_args(
    args: &Args,
) -> Result<(
    gwas_utils::Reader,
    gwas_utils::Writer,
    ColumnSelection,
    char,
)> {
    let mut file_rdr = open_reader(&args.input)?;
    let file_wtr = open_writer(&args.output)?;
    let sep = match args.delim.as_str() {
        "auto" => file_rdr.sniff_csv_delimiter()?,
        _ => get_delimeter_from_cli_argument(&args.delim)?,
    };
    let column_selection = if !args.columns.is_empty() {
        ColumnSelection::Names(args.columns.clone())
    } else if !args.indices.is_empty() {
        ColumnSelection::Indices(args.indices.clone())
    } else if !args.regexes.is_empty() {
        ColumnSelection::Regexes(args.regexes.clone())
    } else {
        return Err(gwas_utils::GuError::Message(
            "Must specify columns, indices, or regex to select".to_string(),
        ));
    };
    Ok((file_rdr, file_wtr, column_selection, sep))
}

enum ColumnSelection {
    Names(Vec<String>),
    Indices(Vec<usize>),
    Regexes(Vec<String>),
}

fn get_column_indices_to_select(
    c: ColumnSelection,
    h: &csv::StringRecord,
    no_reorder: bool,
) -> Result<Vec<usize>> {
    let mut indices = match c {
        ColumnSelection::Names(names) => names
            .iter()
            .map(|name| get_column_idx_from_name(h, name))
            .collect::<Result<Vec<usize>>>(),
        ColumnSelection::Indices(indices) => indices
            .iter()
            .map(|&idx| {
                if idx > h.len() {
                    Err(column_idx_out_of_bounds_error())
                } else {
                    Ok(idx - 1)
                }
            })
            .collect::<Result<Vec<usize>>>(),
        ColumnSelection::Regexes(regexes) => {
            let indices = get_column_indices_from_regexes(h, regexes)?;
            if indices.is_empty() {
                return Err(gwas_utils::GuError::Message(
                    "No columns matched the provided regexes".to_string(),
                ));
            } else {
                Ok(indices)
            }
        }
    }?;
    if no_reorder {
        indices.sort_unstable();
    }
    Ok(indices)
}

fn process_file<R, W>(
    rdr: R,
    wtr: W,
    column_selection: ColumnSelection,
    no_reorder: bool,
    sep: char,
) -> Result<()>
where
    R: io::Read,
    W: io::Write,
{
    let mut csv_rdr = get_csv_reader(rdr, sep);
    let header = csv_rdr.headers()?.clone();

    let column_indices_to_retain =
        get_column_indices_to_select(column_selection, &header, no_reorder)?;

    let header_reduced = column_indices_to_retain
        .iter()
        .map(|&i| &header[i])
        .collect::<Vec<_>>();

    let mut csv_wtr = get_csv_writer(wtr, sep);
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
    use std::io::Cursor;

    use super::*;

    const TEST_INPUT: &str = r#"CHROM GENPOS ID ALLELE0 ALLELE1 A1FREQ INFO N TEST BETA SE CHISQ LOG10P EXTRA
1 1 1 2 1 0.214575 1 494 ADD 0.0775674 0.230001 0.113736 0.133163 NA
1 2 2 2 1 0.218623 1 494 ADD 0.131068 0.239808 0.29872 0.233077 NA
1 3 3 2 1 0.211538 1 494 ADD -0.256723 0.244611 1.10148 0.531739 NA
1 4 4 2 1 0.191296 1 494 ADD -0.131175 0.250523 0.274164 0.221449 NA
1 5 5 2 1 0.195344 1 494 ADD -0.187228 0.235372 0.632751 0.370236 NA
"#;

    const REORDERED_RESULT: &str = r#"ID CHROM LOG10P
1 1 0.133163
2 1 0.233077
3 1 0.531739
4 1 0.221449
5 1 0.370236
"#;

    const NOREORDER_RESULT: &str = r#"CHROM ID LOG10P
1 1 0.133163
1 2 0.233077
1 3 0.531739
1 4 0.221449
1 5 0.370236
"#;

    fn run_select(column_selection: ColumnSelection, no_reorder: bool) -> Result<String> {
        let mut wtr = Cursor::new(Vec::new());
        process_file(
            Cursor::new(TEST_INPUT.as_bytes()),
            &mut wtr,
            column_selection,
            no_reorder,
            ' ',
        )?;
        Ok(String::from_utf8(wtr.into_inner()).unwrap())
    }

    #[test]
    fn test_select_columns_noreorder() {
        let result = run_select(
            ColumnSelection::Names(vec!["ID".into(), "CHROM".into(), "LOG10P".into()]),
            true,
        )
        .unwrap();
        assert_eq!(result, NOREORDER_RESULT);
    }

    #[test]
    fn test_select_columns_reorder() {
        let result = run_select(
            ColumnSelection::Names(vec!["ID".into(), "CHROM".into(), "LOG10P".into()]),
            false,
        )
        .unwrap();
        assert_eq!(result, REORDERED_RESULT);
    }

    #[test]
    fn test_select_columns_column_not_found() {
        let result = run_select(
            ColumnSelection::Names(vec!["ID".into(), "CHR".into(), "LOG10P".into()]),
            false,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_select_indices_reorder() {
        // ID=3, CHROM=1, LOG10P=123 (1-based indices)
        let result = run_select(ColumnSelection::Indices(vec![3, 1, 13]), false).unwrap();
        assert_eq!(result, REORDERED_RESULT);
    }

    #[test]
    fn test_select_indices_noreorder() {
        let result = run_select(ColumnSelection::Indices(vec![3, 1, 13]), true).unwrap();
        assert_eq!(result, NOREORDER_RESULT);
    }

    #[test]
    fn test_select_indices_out_of_bounds() {
        let result = run_select(ColumnSelection::Indices(vec![3, 1, 100]), false);
        assert!(result.is_err());
    }

    #[test]
    fn test_select_regexes_reorder() {
        let result = run_select(
            ColumnSelection::Regexes(vec!["^ID$".into(), "^CHROM$".into(), "^LOG10P$".into()]),
            false,
        )
        .unwrap();
        assert_eq!(result, REORDERED_RESULT);
    }

    #[test]
    fn test_select_regexes_noreorder() {
        let result = run_select(
            ColumnSelection::Regexes(vec!["^ID$".into(), "^CHROM$".into(), "^LOG10P$".into()]),
            true,
        )
        .unwrap();
        assert_eq!(result, NOREORDER_RESULT);
    }

    #[test]
    fn test_select_regexes_not_found() {
        let result = run_select(
            ColumnSelection::Regexes(vec!["^SILLY$".into(), "^NOPE$".into()]),
            false,
        );
        assert!(result.is_err());
    }
}
