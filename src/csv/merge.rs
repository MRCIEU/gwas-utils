use clap::Parser;
use std::collections::HashMap;
use std::io;

use gwas_utils::{GuError, Result, get_delimeter_from_cli_argument, open_reader, open_writer};

use crate::csv::lib::{
    get_column_idx_from_name, get_column_value_from_idx, get_csv_reader, get_csv_writer,
};

pub(crate) const ABOUT: &str =
    "Merge two CSV files based on a shared column. Duplicate keys are not handled sensibly";
pub(crate) const USAGE: &str =
    "gu csv merge infile1.csv[.gz] infile2.csv[.gz] -c KEY_COLUMN [-o outfile.csv[.gz]]";

pub(crate) fn get_usage() -> String {
    USAGE.to_string()
}

#[derive(Debug, Clone, PartialEq)]
enum JoinType {
    Inner,
    Left,
    Right,
    Outer,
}

impl std::str::FromStr for JoinType {
    type Err = GuError;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "inner" => Ok(JoinType::Inner),
            "left" => Ok(JoinType::Left),
            "right" => Ok(JoinType::Right),
            "outer" => Ok(JoinType::Outer),
            _ => Err(GuError::Message(format!("Unknown join type: {s}"))),
        }
    }
}

#[derive(Parser, Debug)]
pub(crate) struct Args {
    /// CSV files to merge (can be gzipped if filenames end with .gz). Right-hand file is streamed from disk for --join=inner and --join=right
    #[arg(num_args = 2, required = true)]
    input: Vec<String>,

    /// Column name to merge on
    #[arg(short, long, num_args = 1, required = true)]
    column: String,

    /// Join type
    #[arg(short, long, num_args = 1, required = false, default_value = "inner", value_parser = clap::builder::PossibleValuesParser::new(
         ["inner", "outer", "left", "right"]
     ),)]
    join: String,

    /// Column name to merge on for second file, if different
    #[arg(long)]
    c2: Option<String>,

    /// Delimiter for CSV file reading and writing
    #[arg(long, default_value = "auto")]
    d1: String,

    /// Delimiter for the second CSV file
    #[arg(long, default_value = "auto")]
    d2: String,

    /// Fill string for missing values in unmatched rows (left, right, outer joins)
    #[arg(short, long, default_value = "")]
    fill: String,

    /// CSV file to write (will be gzipped if filename ends with .gz)
    #[arg(short, long, default_value = "stdout")]
    output: String,
}

pub(crate) fn run(args: Args) -> Result<()> {
    let (file_rdr1, file_rdr2, file_wtr, merge_column1, merge_column2, join_type, fill, sep1, sep2) =
        handle_commandline_args(args)?;
    process_files(
        file_rdr1,
        file_rdr2,
        file_wtr,
        merge_column1,
        merge_column2,
        join_type,
        &fill,
        sep1,
        sep2,
    )
}

fn handle_commandline_args(
    args: Args,
) -> Result<(
    gwas_utils::Reader,
    gwas_utils::Reader,
    gwas_utils::Writer,
    String,
    Option<String>,
    JoinType,
    String,
    char,
    char,
)> {
    let mut file_rdr1 = open_reader(&args.input[0])?;
    let mut file_rdr2 = open_reader(&args.input[1])?;
    let file_wtr = open_writer(&args.output)?;
    let sep1 = match args.d1.as_str() {
        "auto" => file_rdr1.sniff_csv_delimiter()?,
        _ => get_delimeter_from_cli_argument(&args.d1)?,
    };
    let sep2 = match args.d2.as_str() {
        "auto" => file_rdr2.sniff_csv_delimiter()?,
        _ => get_delimeter_from_cli_argument(&args.d2)?,
    };
    let join_type: JoinType = args.join.parse()?;
    Ok((
        file_rdr1,
        file_rdr2,
        file_wtr,
        args.column,
        args.c2,
        join_type,
        args.fill,
        sep1,
        sep2,
    ))
}

fn process_files<R, W>(
    rdr1: R,
    rdr2: R,
    wtr: W,
    merge_column1: String,
    merge_column2: Option<String>,
    join_type: JoinType,
    fill: &str,
    sep1: char,
    sep2: char,
) -> Result<()>
where
    R: io::Read,
    W: io::Write,
{
    let merge_column2 = merge_column2.unwrap_or_else(|| merge_column1.clone());

    // Inner and right joins only need file1 in memory; file2 can be streamed.
    // Left and outer joins must load both files to iterate file1 with file2 lookups.
    match join_type {
        JoinType::Inner | JoinType::Right => {
            let (header1, map1, _) = load_csv(rdr1, sep1, &merge_column1)?;
            let key_column_idx1 = get_column_idx_from_name(&header1, &merge_column1)?;
            let file1_width = header1.len();

            let mut csv_rdr2 = get_csv_reader(rdr2, sep2);
            let header2 = csv_rdr2.headers()?.clone();
            let key_column_idx2 = get_column_idx_from_name(&header2, &merge_column2)?;

            let combined_header = combine_records(header1.clone(), header2, key_column_idx2)?;
            let mut csv_wtr = get_csv_writer(wtr, sep1);
            csv_wtr.write_record(&combined_header)?;

            for result in csv_rdr2.records() {
                let record = result?;
                let key_value = get_column_value_from_idx(&record, key_column_idx2)?;
                match (map1.get(key_value), &join_type) {
                    (Some(r1), _) => {
                        csv_wtr.write_record(&combine_records(
                            r1.clone(),
                            record,
                            key_column_idx2,
                        )?)?;
                    }
                    (None, JoinType::Right) => {
                        csv_wtr.write_record(&make_right_only_row(
                            key_value,
                            &record,
                            key_column_idx1,
                            file1_width,
                            key_column_idx2,
                            fill,
                        ))?;
                    }
                    (None, _) => {} // inner join: no match, skip
                }
            }
        }
        JoinType::Left | JoinType::Outer => {
            let (header1, map1, keys1) = load_csv(rdr1, sep1, &merge_column1)?;
            let (header2, map2, keys2) = load_csv(rdr2, sep2, &merge_column2)?;

            let key_column_idx1 = get_column_idx_from_name(&header1, &merge_column1)?;
            let key_column_idx2 = get_column_idx_from_name(&header2, &merge_column2)?;

            let combined_header =
                combine_records(header1.clone(), header2.clone(), key_column_idx2)?;
            let file1_width = header1.len();
            let file2_extra_width = header2.len() - 1;

            let mut csv_wtr = get_csv_writer(wtr, sep1);
            csv_wtr.write_record(&combined_header)?;

            // All rows from file1, with empty file2 fields where there is no match
            for key in &keys1 {
                let r1 = map1.get(key).unwrap();
                match map2.get(key) {
                    Some(r2) => {
                        csv_wtr.write_record(&combine_records(
                            r1.clone(),
                            r2.clone(),
                            key_column_idx2,
                        )?)?;
                    }
                    None => {
                        let mut row = r1.clone();
                        for _ in 0..file2_extra_width {
                            row.push_field(fill);
                        }
                        csv_wtr.write_record(&row)?;
                    }
                }
            }

            // Outer only: append rows present only in file2
            if join_type == JoinType::Outer {
                for key in &keys2 {
                    if !map1.contains_key(key) {
                        let r2 = map2.get(key).unwrap();
                        csv_wtr.write_record(&make_right_only_row(
                            key,
                            r2,
                            key_column_idx1,
                            file1_width,
                            key_column_idx2,
                            fill,
                        ))?;
                    }
                }
            }
        }
    }

    Ok(())
}

/// Builds an output row for a key that exists only in file2 (used by right and outer joins).
/// The key value is placed at `key_column_idx1` in the output; all other file1 columns use `fill`.
fn make_right_only_row(
    key: &str,
    r2: &csv::StringRecord,
    key_column_idx1: usize,
    file1_width: usize,
    key_column_idx2: usize,
    fill: &str,
) -> csv::StringRecord {
    let mut row = csv::StringRecord::new();
    for i in 0..file1_width {
        if i == key_column_idx1 {
            row.push_field(key);
        } else {
            row.push_field(fill);
        }
    }
    for (i, field) in r2.iter().enumerate() {
        if i != key_column_idx2 {
            row.push_field(field);
        }
    }
    row
}

fn load_csv<R>(
    rdr: R,
    sep: char,
    key: &str,
) -> Result<(
    csv::StringRecord,
    HashMap<String, csv::StringRecord>,
    Vec<String>,
)>
where
    R: io::Read,
{
    let mut csv_rdr = get_csv_reader(rdr, sep);
    let header = csv_rdr.headers()?.clone();
    let key_column_idx = get_column_idx_from_name(&header, key)?;

    let mut map = HashMap::new();
    let mut keys = Vec::new();

    for result in csv_rdr.records() {
        let record = result?;
        let key_value = get_column_value_from_idx(&record, key_column_idx)?.to_string();
        if !map.contains_key(&key_value) {
            keys.push(key_value.clone());
        }
        map.insert(key_value, record);
    }

    Ok((header, map, keys))
}

fn combine_records(
    r1: csv::StringRecord,
    r2: csv::StringRecord,
    key_column_idx: usize,
) -> Result<csv::StringRecord> {
    let mut combined = r1.clone();
    for (i, field) in r2.iter().enumerate() {
        if i != key_column_idx {
            combined.push_field(field);
        }
    }
    Ok(combined)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- combine_records ---

    #[test]
    fn test_combine_records_drops_key_column() {
        // Key column is at index 0 in r2; it should be dropped from the output
        let r1 = csv::StringRecord::from(vec!["A", "1"]);
        let r2 = csv::StringRecord::from(vec!["A", "101"]);
        let combined = combine_records(r1, r2, 0).unwrap();
        assert_eq!(combined, csv::StringRecord::from(vec!["A", "1", "101"]));
    }

    #[test]
    fn test_combine_records_key_not_first_column() {
        // Key column is at index 1 in r2; only the non-key columns of r2 are appended
        let r1 = csv::StringRecord::from(vec!["1", "A"]);
        let r2 = csv::StringRecord::from(vec!["101", "A", "extra"]);
        let combined = combine_records(r1, r2, 1).unwrap();
        assert_eq!(
            combined,
            csv::StringRecord::from(vec!["1", "A", "101", "extra"])
        );
    }

    // --- load_csv ---

    #[test]
    fn test_load_csv_header_and_map() {
        let input = r#"ID,VALUE
A,1
B,2
C,3
"#;
        let (header, map, keys) =
            load_csv(std::io::Cursor::new(input.as_bytes()), ',', "ID").unwrap();
        assert_eq!(header, csv::StringRecord::from(vec!["ID", "VALUE"]));
        assert_eq!(keys, vec!["A", "B", "C"]);
        assert_eq!(map["A"], csv::StringRecord::from(vec!["A", "1"]));
        assert_eq!(map["B"], csv::StringRecord::from(vec!["B", "2"]));
        assert_eq!(map["C"], csv::StringRecord::from(vec!["C", "3"]));
    }

    #[test]
    fn test_load_csv_key_not_first_column() {
        // Key column is VALUE, not the first column
        let input = r#"ID,VALUE
1,A
2,B
"#;
        let (header, map, keys) =
            load_csv(std::io::Cursor::new(input.as_bytes()), ',', "VALUE").unwrap();
        assert_eq!(header, csv::StringRecord::from(vec!["ID", "VALUE"]));
        assert_eq!(keys, vec!["A", "B"]);
        assert_eq!(map["A"], csv::StringRecord::from(vec!["1", "A"]));
    }

    #[test]
    fn test_load_csv_duplicate_keys_last_wins() {
        // Duplicate keys: the last record for a given key is kept
        let input = r#"ID,VALUE
A,1
A,2
B,3
"#;
        let (_, map, keys) = load_csv(std::io::Cursor::new(input.as_bytes()), ',', "ID").unwrap();
        // Key order: A appears once (first occurrence), then B
        assert_eq!(keys, vec!["A", "B"]);
        // Last record for A wins
        assert_eq!(map["A"], csv::StringRecord::from(vec!["A", "2"]));
    }

    #[test]
    fn test_load_csv_missing_key_column_errors() {
        let input = r#"ID,VALUE
A,1
"#;
        let result = load_csv(std::io::Cursor::new(input.as_bytes()), ',', "MISSING");
        assert!(result.is_err());
    }

    // --- make_right_only_row ---

    #[test]
    fn test_make_right_only_row_key_at_index_zero() {
        // file1 has 2 columns, key is at index 0; r2's key column (index 0) is dropped
        let r2 = csv::StringRecord::from(vec!["X", "99"]);
        let row = make_right_only_row("X", &r2, 0, 2, 0, "");
        assert_eq!(row, csv::StringRecord::from(vec!["X", "", "99"]));
    }

    #[test]
    fn test_make_right_only_row_key_not_first_column() {
        // file1 has 3 columns, key is at index 1
        let r2 = csv::StringRecord::from(vec!["99", "X", "extra"]);
        let row = make_right_only_row("X", &r2, 1, 3, 1, "");
        assert_eq!(
            row,
            csv::StringRecord::from(vec!["", "X", "", "99", "extra"])
        );
    }

    #[test]
    fn test_make_right_only_row_with_fill() {
        let r2 = csv::StringRecord::from(vec!["X", "99"]);
        let row = make_right_only_row("X", &r2, 0, 2, 0, "NA");
        assert_eq!(row, csv::StringRecord::from(vec!["X", "NA", "99"]));
    }

    // --- integration tests --- //

    fn run_process(
        input1: &str,
        input2: &str,
        col1: &str,
        col2: Option<&str>,
        join_type: JoinType,
        fill: &str,
    ) -> String {
        let mut wtr = std::io::Cursor::new(Vec::new());
        process_files(
            std::io::Cursor::new(input1.as_bytes()),
            std::io::Cursor::new(input2.as_bytes()),
            &mut wtr,
            col1.to_string(),
            col2.map(|s| s.to_string()),
            join_type,
            fill,
            ',',
            ',',
        )
        .unwrap();
        String::from_utf8(wtr.into_inner()).unwrap()
    }

    #[test]
    fn test_inner_same_column_name() {
        let input1 = r#"ID,VALUE1
A,1
B,2
C,3
D,4
E,5
"#;
        let input2 = r#"ID,VALUE2
A,101
B,102
C,103
D,104
E,105
"#;
        let result = run_process(input1, input2, "ID", None, JoinType::Inner, "");
        assert_eq!(
            result,
            r#"ID,VALUE1,VALUE2
A,1,101
B,2,102
C,3,103
D,4,104
E,5,105
"#
        );
    }

    #[test]
    fn test_inner_different_column_names() {
        let input1 = r#"ID,VALUE1
A,1
B,2
C,3
D,4
E,5
"#;
        let input2 = r#"EID,VALUE2
A,101
B,102
C,103
D,104
E,105
"#;
        let result = run_process(input1, input2, "ID", Some("EID"), JoinType::Inner, "");
        assert_eq!(
            result,
            r#"ID,VALUE1,VALUE2
A,1,101
B,2,102
C,3,103
D,4,104
E,5,105
"#
        );
    }

    #[test]
    fn test_inner_partial_overlap() {
        let input1 = r#"ID,VALUE1
A,1
B,2
C,3
"#;
        let input2 = r#"ID,VALUE2
B,102
C,103
D,104
"#;
        let result = run_process(input1, input2, "ID", None, JoinType::Inner, "");
        // Only B and C appear in both
        assert_eq!(
            result,
            r#"ID,VALUE1,VALUE2
B,2,102
C,3,103
"#
        );
    }

    #[test]
    fn test_left_partial_overlap() {
        let input1 = r#"ID,VALUE1
A,1
B,2
C,3
"#;
        let input2 = r#"ID,VALUE2
B,102
C,103
D,104
"#;
        let result = run_process(input1, input2, "ID", None, JoinType::Left, "");
        // All of file1; D from file2 is excluded; A has no file2 match so VALUE2 is empty
        assert_eq!(
            result,
            r#"ID,VALUE1,VALUE2
A,1,
B,2,102
C,3,103
"#
        );
    }

    #[test]
    fn test_right_partial_overlap() {
        let input1 = r#"ID,VALUE1
A,1
B,2
C,3
"#;
        let input2 = r#"ID,VALUE2
B,102
C,103
D,104
"#;
        let result = run_process(input1, input2, "ID", None, JoinType::Right, "");
        // All of file2; A from file1 is excluded; D has no file1 match so VALUE1 is empty
        assert_eq!(
            result,
            r#"ID,VALUE1,VALUE2
B,2,102
C,3,103
D,,104
"#
        );
    }

    #[test]
    fn test_outer_partial_overlap() {
        let input1 = r#"ID,VALUE1
A,1
B,2
C,3
"#;
        let input2 = r#"ID,VALUE2
B,102
C,103
E,105
D,104
"#;
        let result = run_process(input1, input2, "ID", None, JoinType::Outer, "");
        // All keys from both files; A has no VALUE2, D has no VALUE1
        assert_eq!(
            result,
            r#"ID,VALUE1,VALUE2
A,1,
B,2,102
C,3,103
E,,105
D,,104
"#
        );
    }

    #[test]
    fn test_fill_value_outer() {
        let input1 = r#"ID,VALUE1
A,1
B,2
C,3
"#;
        let input2 = r#"ID,VALUE2
B,102
C,103
D,104
"#;
        let result = run_process(input1, input2, "ID", None, JoinType::Outer, "NA");
        assert_eq!(
            result,
            r#"ID,VALUE1,VALUE2
A,1,NA
B,2,102
C,3,103
D,NA,104
"#
        );
    }

    #[test]
    fn test_fill_value_left() {
        let input1 = r#"ID,VALUE1
A,1
B,2
C,3
"#;
        let input2 = r#"ID,VALUE2
B,102
C,103
D,104
"#;
        let result = run_process(input1, input2, "ID", None, JoinType::Left, "NA");
        // A has no match in file2, so VALUE2 should be "NA"; D from file2 is excluded entirely
        assert_eq!(
            result,
            r#"ID,VALUE1,VALUE2
A,1,NA
B,2,102
C,3,103
"#
        );
    }

    #[test]
    fn test_fill_value_right() {
        let input1 = r#"ID,VALUE1
A,1
B,2
C,3
"#;
        let input2 = r#"ID,VALUE2
B,102
C,103
D,104
"#;
        let result = run_process(input1, input2, "ID", None, JoinType::Right, "NA");
        // D has no match in file1, so VALUE1 should be "NA"; A from file1 is excluded entirely
        assert_eq!(
            result,
            r#"ID,VALUE1,VALUE2
B,2,102
C,3,103
D,NA,104
"#
        );
    }
}
