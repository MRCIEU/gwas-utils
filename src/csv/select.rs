use clap::Parser;
use std::io;

use gwas_utils::{Result, get_delimeter_from_cli_argument, open_reader, open_writer};

use crate::csv::lib::{column_not_found_error, get_csv_reader, get_csv_writer};

pub(crate) const ABOUT: &str = "Select specific columns from a CSV file";
pub(crate) const USAGE: &str =
    "gu csv select infile.csv[.gz] -c <column1 column2 ...> [-o outfile.csv[.gz]]";

pub(crate) fn get_usage() -> String {
    USAGE.to_string()
}

#[derive(Parser, Debug)]
pub(crate) struct Args {
    /// CSV file to process (can be gzipped if filename ends with .gz)
    #[arg(default_value = "stdin")]
    input: String,

    /// Column names to select
    #[arg(short, long, num_args = 1.., required = true)]
    columns: Vec<String>,

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
    let (file_rdr, file_wtr, columns_to_select, no_reorder, sep) = handle_commandline_args(args)?;
    process_file(file_rdr, file_wtr, columns_to_select, no_reorder, sep)
}

fn handle_commandline_args(
    args: Args,
) -> Result<(
    gwas_utils::Reader,
    gwas_utils::Writer,
    Vec<String>,
    bool,
    char,
)> {
    let mut file_rdr = open_reader(&args.input)?;
    let file_wtr = open_writer(&args.output)?;
    let sep = match args.delim.as_str() {
        "auto" => file_rdr.sniff_csv_delimiter()?,
        _ => get_delimeter_from_cli_argument(&args.delim)?,
    };
    Ok((file_rdr, file_wtr, args.columns, args.no_reorder, sep))
}

fn process_file<R, W>(
    rdr: R,
    wtr: W,
    columns_to_select: Vec<String>,
    no_reorder: bool,
    sep: char,
) -> Result<()>
where
    R: io::Read,
    W: io::Write,
{
    let mut csv_rdr = get_csv_reader(rdr, sep);
    let header = csv_rdr.headers()?.clone();

    for column in &columns_to_select {
        if !header.iter().any(|h| h == column) {
            return Err(column_not_found_error(column));
        }
    }

    let column_indices_to_retain: Vec<usize> = if no_reorder {
        header
            .iter()
            .enumerate()
            .filter_map(|(i, s)| columns_to_select.contains(&s.to_string()).then_some(i))
            .collect()
    } else {
        columns_to_select
            .iter()
            .filter_map(|col| header.iter().position(|h| h == col))
            .collect()
    };

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

    #[test]
    fn test_select_columns_noreorder() {
        let input = r#"CHROM GENPOS ID ALLELE0 ALLELE1 A1FREQ INFO N TEST BETA SE CHISQ LOG10P EXTRA
1 1 1 2 1 0.214575 1 494 ADD 0.0775674 0.230001 0.113736 0.133163 NA
1 2 2 2 1 0.218623 1 494 ADD 0.131068 0.239808 0.29872 0.233077 NA
1 3 3 2 1 0.211538 1 494 ADD -0.256723 0.244611 1.10148 0.531739 NA
1 4 4 2 1 0.191296 1 494 ADD -0.131175 0.250523 0.274164 0.221449 NA
1 5 5 2 1 0.195344 1 494 ADD -0.187228 0.235372 0.632751 0.370236 NA
"#;

        let mut wtr = Cursor::new(Vec::new());

        process_file(
            Cursor::new(input.as_bytes()),
            &mut wtr,
            vec!["ID".into(), "CHROM".into(), "LOG10P".into()],
            true,
            ' ',
        )
        .unwrap();

        let desired_result = r#"CHROM ID LOG10P
1 1 0.133163
1 2 0.233077
1 3 0.531739
1 4 0.221449
1 5 0.370236
"#;

        let result = String::from_utf8(wtr.into_inner()).unwrap();
        assert_eq!(result, desired_result);
    }

    #[test]
    fn test_select_columns_reorder() {
        let input = r#"CHROM GENPOS ID ALLELE0 ALLELE1 A1FREQ INFO N TEST BETA SE CHISQ LOG10P EXTRA
1 1 1 2 1 0.214575 1 494 ADD 0.0775674 0.230001 0.113736 0.133163 NA
1 2 2 2 1 0.218623 1 494 ADD 0.131068 0.239808 0.29872 0.233077 NA
1 3 3 2 1 0.211538 1 494 ADD -0.256723 0.244611 1.10148 0.531739 NA
1 4 4 2 1 0.191296 1 494 ADD -0.131175 0.250523 0.274164 0.221449 NA
1 5 5 2 1 0.195344 1 494 ADD -0.187228 0.235372 0.632751 0.370236 NA
"#;

        let mut wtr = Cursor::new(Vec::new());

        process_file(
            Cursor::new(input.as_bytes()),
            &mut wtr,
            vec!["ID".into(), "CHROM".into(), "LOG10P".into()],
            false,
            ' ',
        )
        .unwrap();

        let desired_result = r#"ID CHROM LOG10P
1 1 0.133163
2 1 0.233077
3 1 0.531739
4 1 0.221449
5 1 0.370236
"#;

        let result = String::from_utf8(wtr.into_inner()).unwrap();
        assert_eq!(result, desired_result);
    }

    #[test]
    fn test_select_columns_column_not_found() {
        let input = r#"CHROM GENPOS ID ALLELE0 ALLELE1 A1FREQ INFO N TEST BETA SE CHISQ LOG10P EXTRA
1 1 1 2 1 0.214575 1 494 ADD 0.0775674 0.230001 0.113736 0.133163 NA
1 2 2 2 1 0.218623 1 494 ADD 0.131068 0.239808 0.29872 0.233077 NA
1 3 3 2 1 0.211538 1 494 ADD -0.256723 0.244611 1.10148 0.531739 NA
1 4 4 2 1 0.191296 1 494 ADD -0.131175 0.250523 0.274164 0.221449 NA
1 5 5 2 1 0.195344 1 494 ADD -0.187228 0.235372 0.632751 0.370236 NA
"#;

        let mut wtr = Cursor::new(Vec::new());

        let e = process_file(
            Cursor::new(input.as_bytes()),
            &mut wtr,
            vec!["ID".into(), "CHR".into(), "LOG10P".into()],
            false,
            ' ',
        );

        if !e.is_err() {
            panic!("Expected error for missing column, but got Ok");
        }
    }
}
