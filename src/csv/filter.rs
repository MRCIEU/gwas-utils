use clap::Parser;
use std::io;

use gwas_utils::{GuError, Result, get_delimeter_from_cli_argument, open_reader, open_writer};

use crate::csv::lib::{
    get_column_idx_from_name, get_column_value_from_idx, get_csv_reader, get_csv_writer,
};

pub(crate) const ABOUT: &str = "Filter rows from a CSV file based on column-specific expressions";
pub(crate) const USAGE: &str = r#"
    gu csv filter infile.csv[.gz] -e 'sex == male' 'age > 5' ... [-o outfile.csv[.gz]]
    gu csv filter infile.csv[.gz] -c ALLELE1 -r "^[ACGT]$" [-o outfile.csv[.gz]]"#;

pub(crate) fn get_usage() -> String {
    USAGE.to_string()
}

#[derive(Parser, Debug)]
pub(crate) struct Args {
    /// CSV file to process (can be gzipped if filename ends with .gz)
    #[arg(default_value = "stdin")]
    input: String,

    /// Expression(s) to filter rows, in the format "COLUMN-NAME OPERATOR VALUE". Possible operators are: "==", "!=", ">=", "<=", ">", "<"
    #[arg(short, long, num_args=1.., conflicts_with = "regex", conflicts_with = "column")]
    expression: Vec<String>,

    /// Rows will be included in the output if any expression is true (default is to include rows only if all expressions are true)
    #[arg(long, default_value_t = false)]
    any: bool,

    /// A single column name whose values will be matched against a single regular expression
    #[arg(short, long, requires = "regex")]
    column: Option<String>,

    /// Regular expression to apply (to the values in the column specified by -c)
    #[arg(short, long, requires = "column")]
    regex: Option<String>,

    /// Invert the logic of the filter, i.e. keep rows that do NOT match the expression(s) or regex
    #[arg(short = 'v', long = "invert", default_value_t = false)]
    invert: bool,

    /// Delimiter for CSV file reading and writing
    #[arg(short, long, default_value = "auto")]
    delim: String,

    /// CSV file to write (will be gzipped if filename ends with .gz)
    #[arg(short, long, default_value = "stdout")]
    output: String,
}

pub(crate) fn run(args: Args) -> Result<()> {
    let (file_rdr, file_wtr, sep, filters, any, invert) = handle_commandline_args(args)?;
    process_file(file_rdr, file_wtr, sep, filters, any, invert)
}

enum RowFilter {
    Expressions(Vec<ColumnExpression>),
    Regex(ColumnRegex),
}

struct ColumnExpression {
    column_name: String,
    column_idx: usize,
    operator: Operator,
    value: String,
}

struct ColumnRegex {
    column_name: String,
    column_idx: usize,
    regex: regex::Regex,
}

fn handle_commandline_args(
    args: Args,
) -> Result<(
    gwas_utils::Reader,
    gwas_utils::Writer,
    char,
    RowFilter,
    bool,
    bool,
)> {
    let mut file_rdr = open_reader(&args.input)?;
    let file_wtr = open_writer(&args.output)?;
    let sep = match args.delim.as_str() {
        "auto" => file_rdr.sniff_csv_delimiter()?,
        _ => get_delimeter_from_cli_argument(&args.delim)?,
    };
    let filters = if !args.expression.is_empty() {
        RowFilter::Expressions(parse_expressions(args.expression)?)
    } else if let Some(ref regex_str) = args.regex {
        let regex = regex::Regex::new(regex_str)?;
        match args.column {
            Some(ref col_name) => RowFilter::Regex(ColumnRegex {
                column_name: col_name.clone(),
                column_idx: 0, // This MUST be set later
                regex,
            }),
            _ => unreachable!(), // This case is already handled by clap's requires attribute
        }
    } else {
        return Err(GuError::Message(
            "Either --filter or --regex (and --column) must be provided".into(),
        ));
    };
    Ok((file_rdr, file_wtr, sep, filters, args.any, args.invert))
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
    fn from_str(op_str: &str) -> Result<Self> {
        match op_str {
            "==" => Ok(Operator::Equal),
            "!=" => Ok(Operator::NotEqual),
            ">" => Ok(Operator::GreaterThan),
            "<" => Ok(Operator::LessThan),
            ">=" => Ok(Operator::GreaterThanOrEqual),
            "<=" => Ok(Operator::LessThanOrEqual),
            _ => Err(GuError::Message(format!(
                "Unsupported operator: {}",
                op_str
            ))),
        }
    }

    fn compare(&self, left: &str, right: &str) -> Result<bool> {
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
                _ => Err(GuError::Message(
                    "Invalid operator for non-numeric values".into(),
                )),
            }
        }
    }
}

fn parse_expressions(expressions: Vec<String>) -> Result<Vec<ColumnExpression>> {
    expressions
        .into_iter()
        .map(|expr| parse_expression(&expr))
        .collect()
}

fn parse_expression(expr: &str) -> Result<ColumnExpression> {
    let operators = ["==", "!=", ">=", "<=", ">", "<"];
    for op in operators {
        if let Some(idx) = expr.find(op) {
            let column_name = expr[..idx].trim();
            let value = expr[idx + op.len()..].trim();
            let operator = Operator::from_str(op)?;
            return Ok(ColumnExpression {
                column_name: column_name.to_string(),
                column_idx: 0, // MUST be set later based on header
                operator,
                value: value.to_string(),
            });
        }
    }
    Err(GuError::Message(format!(
        "Invalid expression format: {}",
        expr
    )))
}

fn process_file<R, W>(
    rdr: R,
    wtr: W,
    sep: char,
    mut filters: RowFilter,
    any: bool,
    invert: bool,
) -> Result<()>
where
    R: io::Read,
    W: io::Write,
{
    let mut csv_rdr = get_csv_reader(rdr, sep);
    let header = csv_rdr.headers()?.clone();
    let mut csv_wtr = get_csv_writer(wtr, sep);

    // Set column indices for filters based on header
    match &mut filters {
        RowFilter::Expressions(exprs) => {
            for expr in exprs.iter_mut() {
                expr.column_idx = get_column_idx_from_name(&header, &expr.column_name)?;
            }
        }
        RowFilter::Regex(regex) => {
            regex.column_idx = get_column_idx_from_name(&header, &regex.column_name)?;
        }
    }

    csv_wtr.write_record(&header)?;

    for result in csv_rdr.records() {
        let record = result?;
        let tests: Vec<bool> = match &filters {
            RowFilter::Expressions(exprs) => exprs
                .iter()
                .map(|expr| {
                    let value = get_column_value_from_idx(&record, expr.column_idx)?;
                    expr.operator.compare(value, &expr.value)
                })
                .collect::<Result<Vec<bool>>>()?,
            RowFilter::Regex(cr) => {
                let value = get_column_value_from_idx(&record, cr.column_idx)?;
                vec![cr.regex.is_match(value)]
            }
        };

        let mut keep_row = if any {
            tests.into_iter().any(|b| b)
        } else {
            tests.into_iter().all(|b| b)
        };
        if invert {
            keep_row = !keep_row;
        }
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

    use regex::Regex;

    use super::*;

    const INPUT_1: &str = r#"CHROM GENPOS ID ALLELE0 ALLELE1 A1FREQ INFO N TEST BETA SE CHISQ LOG10P EXTRA
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
    fn test_filter_csv_expression_1() {
        let mut wtr = Cursor::new(Vec::new());
        let expressions = vec![
            parse_expression("CHROM == 1").unwrap(),
            parse_expression("GENPOS < 3").unwrap(),
        ];
        let filters = RowFilter::Expressions(expressions);
        process_file(
            std::io::Cursor::new(INPUT_1.as_bytes()),
            &mut wtr,
            ' ',
            filters,
            false,
            false,
        )
        .unwrap();

        let desired_result = r#"CHROM GENPOS ID ALLELE0 ALLELE1 A1FREQ INFO N TEST BETA SE CHISQ LOG10P EXTRA
1 1 1 2 1 0.214575 1 494 ADD 0.0775674 0.230001 0.113736 0.133163 NA
1 2 2 2 1 0.218623 1 494 ADD 0.131068 0.239808 0.29872 0.233077 NA
"#;

        let result_str = String::from_utf8(wtr.into_inner()).unwrap();
        assert_eq!(result_str, desired_result);
    }

    #[test]
    fn test_filter_csv_expression_2() {
        let mut wtr = Cursor::new(Vec::new());
        let expressions = vec![
            parse_expression("CHROM == 1").unwrap(),
            parse_expression("GENPOS < 3").unwrap(),
        ];
        let filters = RowFilter::Expressions(expressions);
        process_file(
            std::io::Cursor::new(INPUT_1.as_bytes()),
            &mut wtr,
            ' ',
            filters,
            true,
            false,
        )
        .unwrap();

        let desired_result = r#"CHROM GENPOS ID ALLELE0 ALLELE1 A1FREQ INFO N TEST BETA SE CHISQ LOG10P EXTRA
1 1 1 2 1 0.214575 1 494 ADD 0.0775674 0.230001 0.113736 0.133163 NA
1 2 2 2 1 0.218623 1 494 ADD 0.131068 0.239808 0.29872 0.233077 NA
1 3 3 2 1 0.211538 1 494 ADD -0.256723 0.244611 1.10148 0.531739 NA
1 4 4 2 1 0.191296 1 494 ADD -0.131175 0.250523 0.274164 0.221449 NA
1 5 5 2 1 0.195344 1 494 ADD -0.187228 0.235372 0.632751 0.370236 NA
2 1 5 2 1 0.195344 1 494 ADD -0.187228 0.235372 0.632751 0.370236 NA
2 2 6 2 1 0.190283 1 494 ADD -0.234935 0.245557 0.91536 0.47019 NA
"#;

        let result_str = String::from_utf8(wtr.into_inner()).unwrap();
        assert_eq!(result_str, desired_result);
    }

    #[test]
    fn test_filter_csv_expression_3() {
        let mut wtr = Cursor::new(Vec::new());
        let expressions = vec![
            parse_expression("CHROM == 1").unwrap(),
            parse_expression("GENPOS < 3").unwrap(),
        ];
        let filters = RowFilter::Expressions(expressions);
        process_file(
            std::io::Cursor::new(INPUT_1.as_bytes()),
            &mut wtr,
            ' ',
            filters,
            true,
            true,
        )
        .unwrap();

        let desired_result = r#"CHROM GENPOS ID ALLELE0 ALLELE1 A1FREQ INFO N TEST BETA SE CHISQ LOG10P EXTRA
2 3 7 2 1 0.206478 1 494 ADD 0.11647 0.227747 0.26153 0.215332 NA
2 4 8 2 1 0.188259 1 494 ADD -0.353772 0.251712 1.97533 0.796197 NA
2 5 9 2 1 0.194332 1 494 ADD 0.283254 0.241072 1.38057 0.619781 NA
"#;

        let result_str = String::from_utf8(wtr.into_inner()).unwrap();
        assert_eq!(result_str, desired_result);
    }

    const INPUT_2: &str = r#"CHROM GENPOS ID ALLELE0 ALLELE1 A1FREQ
1 10177 rs367896724 A AC 0.399182
1 10352 rs201106462 T TA 0.393553
1 11008 rs575272151 C G 0.0864556
1 11012 rs544419019 C G 0.0864556
1 13110 rs540538026 G A 0.0586336
1 13116 rs62635286 T G 0.188409
1 13118 rs62028691 A G 0.188409
1 13273 rs531730856 G C 0.133681
1 14464 rs546169444 A T 0.156332
1 14599 rs531646671 T A 0.191031
"#;

    #[test]
    fn test_filter_csv_regex_1() {
        let mut wtr = Cursor::new(Vec::new());
        let regex = ColumnRegex {
            column_name: "ALLELE1".to_string(),
            column_idx: 0, // This MUST be set later
            regex: Regex::new("^[ACGT]$").unwrap(),
        };
        let filters = RowFilter::Regex(regex);
        process_file(
            std::io::Cursor::new(INPUT_2.as_bytes()),
            &mut wtr,
            ' ',
            filters,
            false,
            false,
        )
        .unwrap();

        let desired_result = r#"CHROM GENPOS ID ALLELE0 ALLELE1 A1FREQ
1 11008 rs575272151 C G 0.0864556
1 11012 rs544419019 C G 0.0864556
1 13110 rs540538026 G A 0.0586336
1 13116 rs62635286 T G 0.188409
1 13118 rs62028691 A G 0.188409
1 13273 rs531730856 G C 0.133681
1 14464 rs546169444 A T 0.156332
1 14599 rs531646671 T A 0.191031
"#;

        let result_str = String::from_utf8(wtr.into_inner()).unwrap();
        assert_eq!(result_str, desired_result);
    }
}
