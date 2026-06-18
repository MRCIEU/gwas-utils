use clap::Parser;
use std::io;

use gwas_utils::{Result, get_delimeter_from_cli_argument, open_reader, open_writer};

pub(crate) const ABOUT: &str = "Change the delimeter of a CSV file";
pub(crate) const USAGE: &str = "gu csv delim infile.csv[.gz] -d\"\\t\" [-o outfile.tsv[.gz]]";

pub(crate) fn get_usage() -> String {
    USAGE.to_string()
}

#[derive(Parser, Debug)]
pub(crate) struct Args {
    /// CSV file to process (can be gzipped if filename ends with .gz)
    #[arg(default_value = "stdin")]
    input: String,

    /// Delimiter for INPUT CSV file
    #[arg(long, default_value = "auto")]
    din: String,

    /// Delimiter for OUTPUT CSV file
    #[arg(short, long)]
    delim: String,

    /// CSV file to write (will be gzipped if filename ends with .gz)
    #[arg(short, long, default_value = "stdout")]
    output: String,
}

pub(crate) fn run(args: Args) -> Result<()> {
    let (file_rdr, file_wtr, sep_in, sep_out) = handle_commandline_args(args)?;
    process_file(file_rdr, file_wtr, sep_in, sep_out)
}

fn handle_commandline_args(
    args: Args,
) -> Result<(gwas_utils::Reader, gwas_utils::Writer, char, char)> {
    let mut file_rdr = open_reader(&args.input)?;
    let file_wtr = open_writer(&args.output)?;
    let sep_in = match args.din.as_str() {
        "auto" => file_rdr.sniff()?,
        _ => get_delimeter_from_cli_argument(&args.din)?,
    };
    let sep_out = get_delimeter_from_cli_argument(&args.delim)?;
    Ok((file_rdr, file_wtr, sep_in, sep_out))
}

fn process_file<R, W>(rdr: R, wtr: W, sep_in: char, sep_out: char) -> Result<()>
where
    R: io::Read,
    W: io::Write,
{
    let mut csv_rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .delimiter(sep_in as u8)
        .from_reader(rdr);

    let header = csv_rdr.headers()?.clone();

    let mut csv_wtr = csv::WriterBuilder::new()
        .delimiter(sep_out as u8)
        .from_writer(wtr);

    csv_wtr.write_record(&header)?;

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
    fn test_redelim() {
        let input = r#"CHROM GENPOS ID ALLELE0 ALLELE1 A1FREQ INFO N TEST BETA SE CHISQ LOG10P EXTRA
1 1 1 2 1 0.214575 1 494 ADD 0.0775674 0.230001 0.113736 0.133163 NA
1 2 2 2 1 0.218623 1 494 ADD 0.131068 0.239808 0.29872 0.233077 NA
1 3 3 2 1 0.211538 1 494 ADD -0.256723 0.244611 1.10148 0.531739 NA
1 4 4 2 1 0.191296 1 494 ADD -0.131175 0.250523 0.274164 0.221449 NA
1 5 5 2 1 0.195344 1 494 ADD -0.187228 0.235372 0.632751 0.370236 NA
"#;

        let mut wtr = Cursor::new(Vec::new());
        process_file(std::io::Cursor::new(input.as_bytes()), &mut wtr, ' ', ',').unwrap();

        let desired_result = r#"CHROM,GENPOS,ID,ALLELE0,ALLELE1,A1FREQ,INFO,N,TEST,BETA,SE,CHISQ,LOG10P,EXTRA
1,1,1,2,1,0.214575,1,494,ADD,0.0775674,0.230001,0.113736,0.133163,NA
1,2,2,2,1,0.218623,1,494,ADD,0.131068,0.239808,0.29872,0.233077,NA
1,3,3,2,1,0.211538,1,494,ADD,-0.256723,0.244611,1.10148,0.531739,NA
1,4,4,2,1,0.191296,1,494,ADD,-0.131175,0.250523,0.274164,0.221449,NA
1,5,5,2,1,0.195344,1,494,ADD,-0.187228,0.235372,0.632751,0.370236,NA
"#;

        let result = String::from_utf8(wtr.into_inner()).unwrap();
        assert_eq!(result, desired_result);
    }
}
