use clap::Parser;
use std::io;

use gwas_utils::{Result, get_delimeter_from_cli_argument, open_reader, open_writer};

pub(crate) const ABOUT: &str =
    "Write a tab separated file with missing data replaced by \"NA\"s";
pub(crate) const USAGE: &str = "gu csv regenify infile.csv[.gz] [-o outfile.tsv[.gz]]";

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

    /// Don't replace -1 with NA
    #[arg(long, default_value_t = false)]
    no_rep_neg_one: bool,

    /// CSV file to write (will be gzipped if filename ends with .gz)
    #[arg(short, long, default_value = "stdout")]
    output: String,
}

pub(crate) fn run(args: Args) -> Result<()> {
    let (file_rdr, file_wtr, no_rep_neg_one, sep) = handle_commandline_args(args)?;
    process_file(file_rdr, file_wtr, no_rep_neg_one, sep)
}

fn handle_commandline_args(
    args: Args,
) -> Result<(gwas_utils::Reader, gwas_utils::Writer, bool, char)> {
    let mut file_rdr = open_reader(&args.input)?;
    let file_wtr = open_writer(&args.output)?;
    let sep = match args.delim.as_str() {
        "auto" => file_rdr.sniff()?,
        _ => get_delimeter_from_cli_argument(&args.delim)?,
    };
    Ok((file_rdr, file_wtr, args.no_rep_neg_one, sep))
}

fn process_file<R, W>(rdr: R, wtr: W, neg_one: bool, sep: char) -> Result<()>
where
    R: io::Read,
    W: io::Write,
{
    let mut csv_rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .delimiter(sep as u8)
        .from_reader(rdr);

    let header = csv_rdr.headers()?.clone();

    let mut csv_wtr = csv::WriterBuilder::new().delimiter(b'\t').from_writer(wtr);

    csv_wtr.write_record(&header)?;

    for result in csv_rdr.records() {
        let original = result?;
        let mut record = csv::StringRecord::new();
        for field in original.iter() {
            if field.trim().is_empty() {
                record.push_field("NA");
                continue;
            }
            if field.trim() == "-1" && !neg_one {
                record.push_field("NA");
                continue;
            }
            record.push_field(field);
        }

        csv_wtr.write_record(&record)?;
    }

    csv_wtr.flush()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Cursor;

    #[test]
    fn test() {
        let input1 = r#"ID,VALUE1
A,-1
B,
C,3
D,4
E,5
"#;

        let mut wtr = std::io::Cursor::new(Vec::new());

        process_file(Cursor::new(input1.as_bytes()), &mut wtr, false, ',').unwrap();

        let mut desired_result_str = "ID	VALUE1
A	NA
B	NA
C	3
D	4
E	5
";

        let result_str = String::from_utf8(wtr.into_inner()).unwrap();

        assert_eq!(result_str, desired_result_str);

        wtr = std::io::Cursor::new(Vec::new());

        process_file(std::io::Cursor::new(input1.as_bytes()), &mut wtr, true, ',').unwrap();

        desired_result_str = "ID	VALUE1
A	-1
B	NA
C	3
D	4
E	5
";

        let result_str = String::from_utf8(wtr.into_inner()).unwrap();

        assert_eq!(result_str, desired_result_str);
    }
}
