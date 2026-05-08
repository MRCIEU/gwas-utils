use clap::Parser;
use std::error::Error;
use std::io;

use gwas_utilities::{get_delimeter, open_reader, open_writer};

pub(crate) const USAGE: &str = "csv_filter_rows -i infile.csv[.gz] -e 'sex == male' -e 'age > 5' ... -d \",\" -o filtered.csv[.gz]";
pub(crate) const ABOUT: &str = "Filter rows from a CSV file based on column-specific expressions";

#[derive(Parser, Debug)]
pub(crate) struct Args {
    /// Input CSV file (can be gzipped if filename ends with .gz)
    #[arg(short, long)]
    input: String,

    /// Expression(s) to filter rows, in the format "COLUMN-NAME OPERATOR VALUE". Possible operators are: "==", "!=", ">=", "<=", ">", "<". Rows evaluating to true will be included in the output. Multiple expressions will be combined with AND logic by default (use --any for OR logic)
    #[arg(short, long, num_args=1..)]
    expression: Vec<String>,

    /// Rows will be included in the output if any expression is true (default is to include rows only if all expressions are true)
    #[arg(long, default_value_t = false)]
    any: bool,

    /// Delimiter for CSV file reading and writing (default is tab, use " " for space, etc.)
    #[arg(short, long, default_value = "\\t")]
    delim: String,

    /// Filtered CSV file to write (will be gzipped if filename ends with .gz)
    #[arg(short, long)]
    output: String,
}

pub(crate) fn run(args: Args) -> Result<(), Box<dyn Error>> {
    let (file_rdr, file_wtr, sep, filters, any) = handle_commandline_args(args)?;
    process_file(file_rdr, file_wtr, sep, filters, any)
}

fn handle_commandline_args(
    args: Args,
) -> Result<
    (
        gwas_utilities::Reader,
        gwas_utilities::Writer,
        char,
        Vec<ColumnFilter>,
        bool,
    ),
    Box<dyn Error>,
> {
    let file_rdr = open_reader(&args.input)?;
    let file_wtr = open_writer(&args.output)?;
    let sep = get_delimeter(&args.delim)?;
    let filters = parse_filters(args.expression)?;
    Ok((file_rdr, file_wtr, sep, filters, args.any))
}

enum Operator {
    Equal,
    NotEqual,
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,
}

impl Operator {
    fn from_str(op_str: &str) -> Result<Self, Box<dyn Error>> {
        match op_str {
            "==" => Ok(Operator::Equal),
            "!=" => Ok(Operator::NotEqual),
            ">" => Ok(Operator::GreaterThan),
            "<" => Ok(Operator::LessThan),
            ">=" => Ok(Operator::GreaterThanOrEqual),
            "<=" => Ok(Operator::LessThanOrEqual),
            _ => Err(format!("Unsupported operator: {}", op_str).into()),
        }
    }

    fn compare(&self, left: &str, right: &str) -> Result<bool, Box<dyn Error>> {
        let left_num = left.parse::<f64>();
        let right_num = right.parse::<f64>();

        if let (Ok(left_val), Ok(right_val)) = (left_num, right_num) {
            match self {
                Operator::Equal => Ok(left_val == right_val),
                Operator::NotEqual => Ok(left_val != right_val),
                Operator::GreaterThan => Ok(left_val > right_val),
                Operator::LessThan => Ok(left_val < right_val),
                Operator::GreaterThanOrEqual => Ok(left_val >= right_val),
                Operator::LessThanOrEqual => Ok(left_val <= right_val),
            }
        } else {
            match self {
                Operator::Equal => Ok(left == right),
                Operator::NotEqual => Ok(left != right),
                _ => Err("Invalid operator for non-numeric values".into()),
            }
        }
    }
}

struct ColumnFilter {
    column_name: String,
    column_idx: usize,
    operator: Operator,
    value: String,
}

fn parse_filters(expressions: Vec<String>) -> Result<Vec<ColumnFilter>, Box<dyn Error>> {
    expressions
        .into_iter()
        .map(|expr| parse_filter(&expr))
        .collect()
}

fn parse_filter(expr: &str) -> Result<ColumnFilter, Box<dyn Error>> {
    let operators = ["==", "!=", ">=", "<=", ">", "<"];
    for op in operators {
        if let Some(idx) = expr.find(op) {
            let column_name = expr[..idx].trim();
            let value = expr[idx + op.len()..].trim();
            let operator = Operator::from_str(op)?;
            return Ok(ColumnFilter {
                column_name: column_name.to_string(),
                column_idx: 0, // will be set later based on header
                operator,
                value: value.to_string(),
            });
        }
    }
    Err(format!("Invalid expression format: {}", expr).into())
}

fn process_file<R, W>(
    rdr: R,
    wtr: W,
    sep: char,
    mut filters: Vec<ColumnFilter>,
    any: bool,
) -> Result<(), Box<dyn Error>>
where
    R: io::Read,
    W: io::Write,
{
    let mut csv_rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .delimiter(sep as u8)
        .from_reader(rdr);

    let header = csv_rdr.headers()?.clone();

    // Set column indices for filters based on header
    for filter in &mut filters {
        filter.column_idx = header
            .iter()
            .position(|h| h == filter.column_name)
            .ok_or(format!(
                "Column '{}' not found in CSV header",
                filter.column_name
            ))?;
    }

    let mut csv_wtr = csv::WriterBuilder::new()
        .delimiter(sep as u8)
        .from_writer(wtr);

    csv_wtr.write_record(&header)?;

    for result in csv_rdr.records() {
        let record = result?;
        let tests: Vec<bool> = filters
            .iter()
            .map(|filter| {
                let value = record
                    .get(filter.column_idx)
                    .ok_or("Column index out of bounds")?;
                filter.operator.compare(value, &filter.value)
            })
            .collect::<Result<Vec<bool>, Box<dyn Error>>>()?;

        let keep_row = if any {
            tests.into_iter().any(|b| b)
        } else {
            tests.into_iter().all(|b| b)
        };

        if keep_row {
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
    fn test_filter_csv_1() {
        let file_rdr = open_reader("testdata/small.concat.regenie").unwrap();
        let mut wtr = Cursor::new(Vec::new());
        let filters = vec![
            parse_filter("CHROM == 1").unwrap(),
            parse_filter("GENPOS < 3").unwrap(),
        ];
        process_file(file_rdr, &mut wtr, ' ', filters, false).unwrap();

        let desired_result =
            r#"CHROM GENPOS ID ALLELE0 ALLELE1 A1FREQ INFO N TEST BETA SE CHISQ LOG10P EXTRA
1 1 1 2 1 0.214575 1 494 ADD 0.0775674 0.230001 0.113736 0.133163 NA
1 2 2 2 1 0.218623 1 494 ADD 0.131068 0.239808 0.29872 0.233077 NA
"#
            .to_string();

        let result_str = String::from_utf8(wtr.into_inner()).unwrap();
        assert_eq!(result_str, desired_result);
    }

    #[test]
    fn test_filter_csv_2() {
        let file_rdr = open_reader("testdata/small.concat.regenie").unwrap();
        let mut wtr = Cursor::new(Vec::new());
        let filters = vec![
            parse_filter("CHROM == 1").unwrap(),
            parse_filter("GENPOS < 3").unwrap(),
        ];
        process_file(file_rdr, &mut wtr, ' ', filters, true).unwrap();

        let desired_result =
            r#"CHROM GENPOS ID ALLELE0 ALLELE1 A1FREQ INFO N TEST BETA SE CHISQ LOG10P EXTRA
1 1 1 2 1 0.214575 1 494 ADD 0.0775674 0.230001 0.113736 0.133163 NA
1 2 2 2 1 0.218623 1 494 ADD 0.131068 0.239808 0.29872 0.233077 NA
1 3 3 2 1 0.211538 1 494 ADD -0.256723 0.244611 1.10148 0.531739 NA
1 4 4 2 1 0.191296 1 494 ADD -0.131175 0.250523 0.274164 0.221449 NA
1 5 5 2 1 0.195344 1 494 ADD -0.187228 0.235372 0.632751 0.370236 NA
2 1 5 2 1 0.195344 1 494 ADD -0.187228 0.235372 0.632751 0.370236 NA
2 2 6 2 1 0.190283 1 494 ADD -0.234935 0.245557 0.91536 0.47019 NA
"#
            .to_string();

        let result_str = String::from_utf8(wtr.into_inner()).unwrap();
        assert_eq!(result_str, desired_result);
    }
}
