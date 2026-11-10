use clap::Parser;
use std::collections::HashMap;
use std::io;

use gwas_utils::{GuError, Result, open_reader, open_writer};

use crate::csv::lib::{
    get_column_idx_from_name, get_column_value_from_idx, get_csv_reader, get_csv_writer,
    get_delimiter_from_cli_argument, is_missing,
};

pub(crate) const ABOUT: &str = "Merge two CSV files based on a shared column";
pub(crate) const USAGE: &str =
    "gu csv merge infile1.csv[.gz] infile2.csv[.gz] -c KEY_COLUMN [-o outfile.csv[.gz]]";

pub(crate) fn get_usage() -> String {
    USAGE.to_string()
}

#[derive(Debug, Clone, Copy, PartialEq)]
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

/// What to do when a key that must be unique appears on more than one row.
// FOLLOW-UP: an `all` policy (keep every row, i.e. full one-to-many / many-to-many joins) was
// deliberately left out for now. See CLAUDE.md.
#[derive(Debug, Clone, Copy, PartialEq)]
enum DuplicatePolicy {
    Error,
    First,
    Last,
}

impl std::str::FromStr for DuplicatePolicy {
    type Err = GuError;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "error" => Ok(DuplicatePolicy::Error),
            "first" => Ok(DuplicatePolicy::First),
            "last" => Ok(DuplicatePolicy::Last),
            _ => Err(GuError::Message(format!("Unknown duplicate policy: {s}"))),
        }
    }
}

#[derive(Parser, Debug)]
pub(crate) struct Args {
    /// CSV files to merge (can be gzipped if filenames end with .gz). The first file is held in memory, so its keys should be unique (see --on-duplicate). The second file is streamed from disk for --join=inner and --join=right (see --strict); keys that repeat in it each produce an output row. Rows with a missing key (empty, NA) never match
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

    /// What to do if a key is repeated in the first file: error, or keep only the first or last row for that key (the kept row stays in the position of the first)
    #[arg(long, default_value = "error", value_parser = clap::builder::PossibleValuesParser::new(
         ["error", "first", "last"]
     ),)]
    on_duplicate: String,

    /// Load both files into memory and apply --on-duplicate to the second file as well as the first
    #[arg(long, default_value_t = false)]
    strict: bool,

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

struct MergeConfig {
    column1: String,
    column2: Option<String>,
    join: JoinType,
    on_duplicate: DuplicatePolicy,
    strict: bool,
    fill: String,
    sep1: char,
    sep2: char,
}

pub(crate) fn run(args: Args) -> Result<()> {
    let (file_rdr1, file_rdr2, file_wtr, config) = handle_commandline_args(args)?;
    process_files(file_rdr1, file_rdr2, file_wtr, &config)
}

fn handle_commandline_args(
    args: Args,
) -> Result<(
    gwas_utils::Reader,
    gwas_utils::Reader,
    gwas_utils::Writer,
    MergeConfig,
)> {
    let mut file_rdr1 = open_reader(&args.input[0])?;
    let mut file_rdr2 = open_reader(&args.input[1])?;
    let file_wtr = open_writer(&args.output)?;
    let sep1 = match args.d1.as_str() {
        "auto" => file_rdr1.sniff_csv_delimiter()?,
        _ => get_delimiter_from_cli_argument(&args.d1)?,
    };
    let sep2 = match args.d2.as_str() {
        "auto" => file_rdr2.sniff_csv_delimiter()?,
        _ => get_delimiter_from_cli_argument(&args.d2)?,
    };
    let config = MergeConfig {
        column1: args.column,
        column2: args.c2,
        join: args.join.parse()?,
        on_duplicate: args.on_duplicate.parse()?,
        strict: args.strict,
        fill: args.fill,
        sep1,
        sep2,
    };
    Ok((file_rdr1, file_rdr2, file_wtr, config))
}

/// A file held in memory, in file order, with a lookup from key to row.
/// Rows with a missing key are kept in `rows` but are not in `index`, so they never match.
struct Table {
    header: csv::StringRecord,
    key_idx: usize,
    rows: Vec<csv::StringRecord>,
    index: HashMap<String, usize>,
}

/// Load a file in which keys must be unique, resolving repeated keys according to `policy`.
/// `label` names the file in error messages.
fn load_table<R>(
    rdr: R,
    sep: char,
    key: &str,
    policy: DuplicatePolicy,
    label: &str,
) -> Result<Table>
where
    R: io::Read,
{
    let mut csv_rdr = get_csv_reader(rdr, sep);
    let header = csv_rdr.headers()?.clone();
    let key_idx = get_column_idx_from_name(&header, key)?;

    let mut rows: Vec<csv::StringRecord> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();

    for result in csv_rdr.records() {
        let record = result?;
        let key_value = get_column_value_from_idx(&record, key_idx)?;
        if is_missing(key_value) {
            rows.push(record);
            continue;
        }
        match index.get(key_value).copied() {
            Some(existing) => match policy {
                DuplicatePolicy::Error => {
                    let line = record.position().map_or(0, |p| p.line());
                    return Err(GuError::Message(format!(
                        "Duplicate key \"{}\" in {} (line {}). Use --on-duplicate first or last to keep a single row per key",
                        key_value, label, line
                    )));
                }
                DuplicatePolicy::First => {}
                DuplicatePolicy::Last => rows[existing] = record,
            },
            None => {
                index.insert(key_value.to_string(), rows.len());
                rows.push(record);
            }
        }
    }

    Ok(Table {
        header,
        key_idx,
        rows,
        index,
    })
}

type RecordIter<'a> = Box<dyn Iterator<Item = Result<csv::StringRecord>> + 'a>;

/// The second file as a header, the index of its key column, and its rows.
/// By default the rows are read lazily from the file. With `--strict` the file is loaded first so
/// that `--on-duplicate` can be applied to it. Either way the rows come out in file order.
fn open_second_file<'a, R>(
    rdr: R,
    config: &MergeConfig,
    key: &str,
) -> Result<(csv::StringRecord, usize, RecordIter<'a>)>
where
    R: io::Read + 'a,
{
    if config.strict {
        let table = load_table(
            rdr,
            config.sep2,
            key,
            config.on_duplicate,
            "the second file",
        )?;
        let rows = table.rows.into_iter().map(Ok);
        Ok((table.header, table.key_idx, Box::new(rows)))
    } else {
        let mut csv_rdr = get_csv_reader(rdr, config.sep2);
        let header = csv_rdr.headers()?.clone();
        let key_idx = get_column_idx_from_name(&header, key)?;
        let rows = csv_rdr.into_records().map(|r| r.map_err(GuError::from));
        Ok((header, key_idx, Box::new(rows)))
    }
}

fn process_files<R, W>(rdr1: R, rdr2: R, wtr: W, config: &MergeConfig) -> Result<()>
where
    R: io::Read,
    W: io::Write,
{
    let key2 = config.column2.as_deref().unwrap_or(&config.column1);
    let table1 = load_table(
        rdr1,
        config.sep1,
        &config.column1,
        config.on_duplicate,
        "the first file",
    )?;

    match config.join {
        JoinType::Inner | JoinType::Right => {
            join_streaming(table1, open_second_file(rdr2, config, key2)?, wtr, config)
        }
        JoinType::Left | JoinType::Outer => join_all_file1_rows(table1, rdr2, wtr, config, key2),
    }
}

/// Inner and right joins: file1 is in memory and each row of file2 is looked up in it,
/// so the output is in file2 order
fn join_streaming<W>(
    table1: Table,
    (header2, key_idx2, records2): (csv::StringRecord, usize, RecordIter<'_>),
    wtr: W,
    config: &MergeConfig,
) -> Result<()>
where
    W: io::Write,
{
    let file1_width = table1.header.len();
    let combined_header = combine_records(table1.header.clone(), header2, key_idx2)?;
    let mut csv_wtr = get_csv_writer(wtr, config.sep1);
    csv_wtr.write_record(&combined_header)?;

    for result in records2 {
        let record = result?;
        let key_value = get_column_value_from_idx(&record, key_idx2)?;
        let match1 = if is_missing(key_value) {
            None
        } else {
            table1.index.get(key_value).map(|&i| &table1.rows[i])
        };
        match (match1, config.join) {
            (Some(r1), _) => {
                csv_wtr.write_record(&combine_records(r1.clone(), record, key_idx2)?)?;
            }
            (None, JoinType::Right) => {
                csv_wtr.write_record(&make_right_only_row(
                    key_value,
                    &record,
                    table1.key_idx,
                    file1_width,
                    key_idx2,
                    &config.fill,
                ))?;
            }
            (None, _) => {} // inner join: no match, skip
        }
    }

    csv_wtr.flush()?;
    Ok(())
}

/// Left and outer joins: every row of file1 is written, so both files are held in memory and the
/// output is in file1 order (followed, for outer joins, by the rows only in file2)
fn join_all_file1_rows<R, W>(
    table1: Table,
    rdr2: R,
    wtr: W,
    config: &MergeConfig,
    key2: &str,
) -> Result<()>
where
    R: io::Read,
    W: io::Write,
{
    let (header2, key_idx2, records2) = open_second_file(rdr2, config, key2)?;
    let rows2 = records2.collect::<Result<Vec<_>>>()?;

    let mut index2: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, row) in rows2.iter().enumerate() {
        let key_value = get_column_value_from_idx(row, key_idx2)?;
        if !is_missing(key_value) {
            index2.entry(key_value).or_default().push(i);
        }
    }
    let mut matched = vec![false; rows2.len()];

    let file1_width = table1.header.len();
    let file2_extra_width = header2.len() - 1;
    let combined_header = combine_records(table1.header.clone(), header2, key_idx2)?;
    let mut csv_wtr = get_csv_writer(wtr, config.sep1);
    csv_wtr.write_record(&combined_header)?;

    // All rows from file1, with fill values where there is no match in file2
    for r1 in &table1.rows {
        let key_value = get_column_value_from_idx(r1, table1.key_idx)?;
        let matches = if is_missing(key_value) {
            None
        } else {
            index2.get(key_value)
        };
        match matches {
            Some(indices) => {
                for &i in indices {
                    matched[i] = true;
                    csv_wtr.write_record(&combine_records(
                        r1.clone(),
                        rows2[i].clone(),
                        key_idx2,
                    )?)?;
                }
            }
            None => {
                let mut row = r1.clone();
                for _ in 0..file2_extra_width {
                    row.push_field(&config.fill);
                }
                csv_wtr.write_record(&row)?;
            }
        }
    }

    // Outer only: append rows present only in file2
    if config.join == JoinType::Outer {
        for (i, r2) in rows2.iter().enumerate() {
            if !matched[i] {
                let key_value = get_column_value_from_idx(r2, key_idx2)?;
                csv_wtr.write_record(&make_right_only_row(
                    key_value,
                    r2,
                    table1.key_idx,
                    file1_width,
                    key_idx2,
                    &config.fill,
                ))?;
            }
        }
    }

    csv_wtr.flush()?;
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

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Args::command().debug_assert();
    }

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

    // --- load_table ---

    fn load(input: &str, key: &str, policy: DuplicatePolicy) -> Result<Table> {
        load_table(
            std::io::Cursor::new(input.as_bytes()),
            ',',
            key,
            policy,
            "test file",
        )
    }

    #[test]
    fn test_load_table_header_rows_and_index() {
        let t = load("ID,VALUE\nA,1\nB,2\nC,3\n", "ID", DuplicatePolicy::Error).unwrap();
        assert_eq!(t.header, csv::StringRecord::from(vec!["ID", "VALUE"]));
        assert_eq!(t.key_idx, 0);
        assert_eq!(t.rows.len(), 3);
        assert_eq!(
            t.rows[t.index["B"]],
            csv::StringRecord::from(vec!["B", "2"])
        );
    }

    #[test]
    fn test_load_table_key_not_first_column() {
        let t = load("ID,VALUE\n1,A\n2,B\n", "VALUE", DuplicatePolicy::Error).unwrap();
        assert_eq!(t.key_idx, 1);
        assert_eq!(
            t.rows[t.index["A"]],
            csv::StringRecord::from(vec!["1", "A"])
        );
    }

    #[test]
    fn test_load_table_duplicate_keys_policies() {
        let input = "ID,VALUE\nA,1\nB,3\nA,2\n";

        let err = load(input, "ID", DuplicatePolicy::Error).err().unwrap();
        let msg = err.to_string();
        assert!(
            msg.contains("\"A\"") && msg.contains("test file") && msg.contains("line 4"),
            "{msg}"
        );

        // First: the first row for the key is kept
        let t = load(input, "ID", DuplicatePolicy::First).unwrap();
        assert_eq!(t.rows.len(), 2);
        assert_eq!(
            t.rows[t.index["A"]],
            csv::StringRecord::from(vec!["A", "1"])
        );

        // Last: the last row is kept, in the position of the first
        let t = load(input, "ID", DuplicatePolicy::Last).unwrap();
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[0], csv::StringRecord::from(vec!["A", "2"]));
        assert_eq!(t.rows[1], csv::StringRecord::from(vec!["B", "3"]));
    }

    #[test]
    fn test_load_table_missing_keys_are_kept_but_not_indexed() {
        // Repeated missing keys aren't duplicates
        let t = load(
            "ID,VALUE\nA,1\nNA,2\n,3\nNA,4\n",
            "ID",
            DuplicatePolicy::Error,
        )
        .unwrap();
        assert_eq!(t.rows.len(), 4);
        assert_eq!(t.index.len(), 1);
    }

    #[test]
    fn test_load_table_missing_key_column_errors() {
        assert!(load("ID,VALUE\nA,1\n", "MISSING", DuplicatePolicy::Error).is_err());
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
        let cfg = test_config(col1, col2, join_type, fill, DuplicatePolicy::Error, false);
        run_with_config(input1, input2, &cfg).unwrap()
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

    // --- duplicate keys, missing keys and --strict --- //

    fn test_config(
        col1: &str,
        col2: Option<&str>,
        join: JoinType,
        fill: &str,
        on_duplicate: DuplicatePolicy,
        strict: bool,
    ) -> MergeConfig {
        MergeConfig {
            column1: col1.to_string(),
            column2: col2.map(|s| s.to_string()),
            join,
            on_duplicate,
            strict,
            fill: fill.to_string(),
            sep1: ',',
            sep2: ',',
        }
    }

    fn run_with_config(input1: &str, input2: &str, config: &MergeConfig) -> Result<String> {
        let mut wtr = std::io::Cursor::new(Vec::new());
        process_files(
            std::io::Cursor::new(input1.as_bytes()),
            std::io::Cursor::new(input2.as_bytes()),
            &mut wtr,
            config,
        )?;
        Ok(String::from_utf8(wtr.into_inner()).unwrap())
    }

    fn run_full(
        input1: &str,
        input2: &str,
        join: JoinType,
        on_duplicate: DuplicatePolicy,
        strict: bool,
    ) -> Result<String> {
        let cfg = test_config("ID", None, join, "NA", on_duplicate, strict);
        run_with_config(input1, input2, &cfg)
    }

    const ALL_JOINS: [JoinType; 4] = [
        JoinType::Inner,
        JoinType::Left,
        JoinType::Right,
        JoinType::Outer,
    ];

    #[test]
    fn test_duplicate_keys_in_file1_are_an_error_by_default_for_every_join() {
        for join in ALL_JOINS {
            for strict in [false, true] {
                let result = run_full(
                    "ID,V1\nA,1\nA,2\n",
                    "ID,V2\nA,9\n",
                    join,
                    DuplicatePolicy::Error,
                    strict,
                );
                assert!(result.is_err(), "{join:?} strict={strict}");
            }
        }
    }

    #[test]
    fn test_duplicate_keys_in_file1_first_and_last() {
        let (f1, f2) = ("ID,V1\nA,1\nB,5\nA,2\n", "ID,V2\nA,9\nB,8\n");
        for join in ALL_JOINS {
            let first = run_full(f1, f2, join, DuplicatePolicy::First, false).unwrap();
            assert!(first.contains("A,1,9"), "{join:?}: {first}");
            let last = run_full(f1, f2, join, DuplicatePolicy::Last, false).unwrap();
            assert!(last.contains("A,2,9"), "{join:?}: {last}");
            // Either way there is exactly one row for A
            assert_eq!(first.matches("A,").count(), 1);
            assert_eq!(last.matches("A,").count(), 1);
        }
    }

    #[test]
    fn test_repeated_keys_in_file2_each_produce_a_row_by_default() {
        let (f1, f2) = ("ID,V1\nA,1\nB,2\n", "ID,V2\nA,9\nA,8\nB,7\n");
        for join in ALL_JOINS {
            let out = run_full(f1, f2, join, DuplicatePolicy::Error, false).unwrap();
            assert!(
                out.contains("A,1,9") && out.contains("A,1,8") && out.contains("B,2,7"),
                "{join:?}: {out}"
            );
        }
    }

    #[test]
    fn test_strict_applies_the_policy_to_file2() {
        let (f1, f2) = ("ID,V1\nA,1\nB,2\n", "ID,V2\nA,9\nA,8\nB,7\n");
        for join in ALL_JOINS {
            assert!(
                run_full(f1, f2, join, DuplicatePolicy::Error, true).is_err(),
                "{join:?}"
            );
            let first = run_full(f1, f2, join, DuplicatePolicy::First, true).unwrap();
            assert!(
                first.contains("A,1,9") && !first.contains("A,1,8"),
                "{join:?}: {first}"
            );
            let last = run_full(f1, f2, join, DuplicatePolicy::Last, true).unwrap();
            assert!(
                last.contains("A,1,8") && !last.contains("A,1,9"),
                "{join:?}: {last}"
            );
        }
    }

    #[test]
    fn test_strict_gives_identical_output_when_there_are_no_duplicates() {
        let f1 = "ID,V1\nA,1\nB,2\nC,3\nNA,4\n";
        let f2 = "ID,V2\nD,104\nC,103\nB,102\n,105\n";
        for join in ALL_JOINS {
            let streaming = run_full(f1, f2, join, DuplicatePolicy::Error, false).unwrap();
            let strict = run_full(f1, f2, join, DuplicatePolicy::Error, true).unwrap();
            assert_eq!(streaming, strict, "{join:?}");
        }
    }

    #[test]
    fn test_missing_keys_never_match() {
        let f1 = "ID,V1\nA,1\nNA,2\n,3\n";
        let f2 = "ID,V2\nA,9\nNA,8\n,7\n";
        for strict in [false, true] {
            let inner = run_full(f1, f2, JoinType::Inner, DuplicatePolicy::Error, strict).unwrap();
            assert_eq!(inner, "ID,V1,V2\nA,1,9\n");

            // Unmatched file1 rows are kept with fill values
            let left = run_full(f1, f2, JoinType::Left, DuplicatePolicy::Error, strict).unwrap();
            assert_eq!(left, "ID,V1,V2\nA,1,9\nNA,2,NA\n,3,NA\n");

            // Unmatched file2 rows are kept, with their own (missing) key
            let right = run_full(f1, f2, JoinType::Right, DuplicatePolicy::Error, strict).unwrap();
            assert_eq!(right, "ID,V1,V2\nA,1,9\nNA,NA,8\n,NA,7\n");

            let outer = run_full(f1, f2, JoinType::Outer, DuplicatePolicy::Error, strict).unwrap();
            assert_eq!(outer, "ID,V1,V2\nA,1,9\nNA,2,NA\n,3,NA\nNA,NA,8\n,NA,7\n");
        }
    }

    #[test]
    fn test_key_column_must_exist_in_both_files() {
        for join in ALL_JOINS {
            let cfg = test_config("ID", Some("NOPE"), join, "", DuplicatePolicy::Error, false);
            assert!(run_with_config("ID,V\nA,1\n", "ID,V\nA,1\n", &cfg).is_err());
        }
    }
}
