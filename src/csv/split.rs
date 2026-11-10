use clap::Parser;
use std::collections::HashMap;
use std::io;
use std::path::Path;

use gwas_utils::{GuError, Result, open_writer_with_gzip_threads};

use crate::csv::lib::{
    get_column_idx_from_name, get_column_value_from_idx, get_csv_reader, open_input,
};

pub(crate) const ABOUT: &str =
    "Split a CSV file into multiple files based on unique values in a specified categorical column";
pub(crate) const USAGE: &str = "gu csv split infile.csv[.gz] -c colname";

pub(crate) fn get_usage() -> String {
    USAGE.to_string()
}

#[derive(Parser, Debug)]
pub(crate) struct Args {
    /// CSV file to process (can be gzipped if filename ends with .gz)
    #[arg(default_value = "stdin")]
    input: String,

    /// Categorical column name to split on. One file is written per distinct value, named
    /// COLNAME.VALUE.SUFFIX. Characters that can't be used in a file name ("/", control
    /// characters) and "%" are percent-encoded (e.g. "a/b" becomes "a%2Fb"). Values that differ
    /// only by case are an error, because their files can't be told apart on some file systems
    #[arg(short, long)]
    column: String,

    /// Delimiter for CSV file reading and writing
    #[arg(short, long, default_value = "auto")]
    delim: String,

    /// Suffix to add to output filenames (by default output files will be named "COLNAME.VAL.csv")
    #[arg(short, long, default_value = "csv")]
    suffix: String,
}

pub(crate) fn run(args: Args) -> Result<()> {
    let (file_rdr, column_to_split_on, sep, suffix) = handle_commandline_args(args)?;
    process_file(file_rdr, column_to_split_on, sep, suffix, Path::new("."))?;
    Ok(())
}

fn handle_commandline_args(args: Args) -> Result<(gwas_utils::Reader, String, char, String)> {
    let (file_rdr, sep) = open_input(&args.input, &args.delim)?;
    let suffix = match args.suffix.is_empty() {
        true => "".to_string(),
        false => format!(".{}", args.suffix),
    };
    Ok((file_rdr, args.column, sep, suffix))
}

/// Compression threads for each gzip output. Every distinct value gets its own output file, all
/// open at once, so a thread pool for each would exhaust the system (60 values once peaked at
/// 541 threads). With 1, gzp compresses on the calling thread and starts no extra threads.
const GZIP_THREADS_PER_OUTPUT: usize = 1;

/// Whether a character can't safely be used as it is in part of a file name
fn needs_encoding(c: char) -> bool {
    c.is_control() || c == '/' || c == '%'
}

/// Percent-encode the characters in `value` that can't be used in a file name, so that every
/// value maps to a different, valid file name ("%" is encoded too, so that the mapping can't
/// produce the same name for two values)
fn encode_file_name_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for c in value.chars() {
        if needs_encoding(c) {
            let mut buf = [0u8; 4];
            for byte in c.encode_utf8(&mut buf).bytes() {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        } else {
            encoded.push(c);
        }
    }
    encoded
}

/// "Too many open files" (EMFILE / ENFILE on Unix, ERROR_TOO_MANY_OPEN_FILES on Windows)
fn is_too_many_open_files(e: &io::Error) -> bool {
    if cfg!(unix) {
        matches!(e.raw_os_error(), Some(23) | Some(24))
    } else if cfg!(windows) {
        e.raw_os_error() == Some(4)
    } else {
        false
    }
}

/// Describe a failure to write to one of the output files, naming the file and, for the
/// common "too many files" case, what to do about it
fn output_error(e: &io::Error, file_name: &str, column: &str, files_open: usize) -> GuError {
    if is_too_many_open_files(e) {
        GuError::Message(format!(
            "Too many open files: splitting on column \"{}\" needs more output files than the system allows to be open at once ({} distinct values had been seen when it failed). Use a column with fewer distinct values",
            column, files_open
        ))
    } else {
        GuError::Message(format!("Couldn't write to {}: {}", file_name, e))
    }
}

fn write_to_output(
    wtr: &mut csv::Writer<gwas_utils::Writer>,
    record: &csv::StringRecord,
    file_name: &str,
    column: &str,
    files_open: usize,
) -> Result<()> {
    wtr.write_record(record).map_err(|e| match e.kind() {
        csv::ErrorKind::Io(io_err) => output_error(io_err, file_name, column, files_open),
        _ => GuError::from(e),
    })
}

struct Output {
    file_name: String,
    writer: csv::Writer<gwas_utils::Writer>,
}

/// Write one file per unique value of the split column into `out_dir`
fn process_file<R>(
    rdr: R,
    column_to_split_on: String,
    sep: char,
    suffix: String,
    out_dir: &Path,
) -> Result<()>
where
    R: io::Read,
{
    let mut csv_rdr = get_csv_reader(rdr, sep);
    let header = csv_rdr.headers()?.clone();
    let column_index = get_column_idx_from_name(&header, &column_to_split_on)?;
    let column_in_file_names = encode_file_name_component(&column_to_split_on);

    let mut outputs: HashMap<String, Output> = HashMap::new();
    // Lower-cased file name -> the value that claimed it, to catch values whose files would be
    // the same file on a case-insensitive file system
    let mut claimed_names: HashMap<String, String> = HashMap::new();

    for result in csv_rdr.records() {
        let record = result?;
        let value = get_column_value_from_idx(&record, column_index)?;

        if !outputs.contains_key(value) {
            let file_name = out_dir
                .join(format!(
                    "{}.{}{}",
                    column_in_file_names,
                    encode_file_name_component(value),
                    suffix
                ))
                .to_string_lossy()
                .into_owned();
            if let Some(other_value) =
                claimed_names.insert(file_name.to_lowercase(), value.to_string())
            {
                return Err(GuError::Message(format!(
                    "The values \"{}\" and \"{}\" would be written to files whose names differ only by case ({}), which can't be told apart on case-insensitive file systems",
                    other_value, value, file_name
                )));
            }
            let mut writer = csv::WriterBuilder::new().delimiter(sep as u8).from_writer(
                open_writer_with_gzip_threads(&file_name, GZIP_THREADS_PER_OUTPUT)?,
            );
            write_to_output(
                &mut writer,
                &header,
                &file_name,
                &column_to_split_on,
                outputs.len(),
            )?;
            outputs.insert(value.to_string(), Output { file_name, writer });
        }

        let files_open = outputs.len();
        if let Some(output) = outputs.get_mut(value) {
            write_to_output(
                &mut output.writer,
                &record,
                &output.file_name,
                &column_to_split_on,
                files_open,
            )?;
        }
    }

    // Flush every file explicitly, so that a write error (e.g. a file that can't be created, or a
    // full disk) is reported instead of being silently ignored when the writer is dropped. Each
    // writer is dropped, closing its file, before the next is flushed, so this doesn't need all
    // the files open at the same time.
    let files_open = outputs.len();
    for (_, mut output) in outputs.drain() {
        output
            .writer
            .flush()
            .map_err(|e| output_error(&e, &output.file_name, &column_to_split_on, files_open))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Args::command().debug_assert();
    }

    #[test]
    fn test_split_file() {
        let input = r#"CHROM GENPOS ID ALLELE0 ALLELE1 A1FREQ INFO N TEST BETA SE CHISQ LOG10P EXTRA
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

        let dir = std::env::temp_dir().join(format!("gu-split-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();

        process_file(
            std::io::Cursor::new(input.as_bytes()),
            "CHROM".to_string(),
            ' ',
            ".csv".to_string(),
            &dir,
        )
        .unwrap();

        let desired_result_1 =
            r#"CHROM GENPOS ID ALLELE0 ALLELE1 A1FREQ INFO N TEST BETA SE CHISQ LOG10P EXTRA
1 1 1 2 1 0.214575 1 494 ADD 0.0775674 0.230001 0.113736 0.133163 NA
1 2 2 2 1 0.218623 1 494 ADD 0.131068 0.239808 0.29872 0.233077 NA
1 3 3 2 1 0.211538 1 494 ADD -0.256723 0.244611 1.10148 0.531739 NA
1 4 4 2 1 0.191296 1 494 ADD -0.131175 0.250523 0.274164 0.221449 NA
1 5 5 2 1 0.195344 1 494 ADD -0.187228 0.235372 0.632751 0.370236 NA
"#
            .to_string();

        let desired_result_2 =
            r#"CHROM GENPOS ID ALLELE0 ALLELE1 A1FREQ INFO N TEST BETA SE CHISQ LOG10P EXTRA
2 1 5 2 1 0.195344 1 494 ADD -0.187228 0.235372 0.632751 0.370236 NA
2 2 6 2 1 0.190283 1 494 ADD -0.234935 0.245557 0.91536 0.47019 NA
2 3 7 2 1 0.206478 1 494 ADD 0.11647 0.227747 0.26153 0.215332 NA
2 4 8 2 1 0.188259 1 494 ADD -0.353772 0.251712 1.97533 0.796197 NA
2 5 9 2 1 0.194332 1 494 ADD 0.283254 0.241072 1.38057 0.619781 NA
"#
            .to_string();

        let result_1 = fs::read_to_string(dir.join("CHROM.1.csv")).unwrap();
        let result_2 = fs::read_to_string(dir.join("CHROM.2.csv")).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(result_1, desired_result_1);
        assert_eq!(result_2, desired_result_2);
    }

    #[test]
    fn test_encode_file_name_component() {
        assert_eq!(encode_file_name_component("1"), "1");
        assert_eq!(
            encode_file_name_component("European ancestry"),
            "European ancestry"
        );
        assert_eq!(encode_file_name_component("chr1:100-200"), "chr1:100-200");
        assert_eq!(encode_file_name_component("Caf\u{e9}"), "Caf\u{e9}");
        assert_eq!(encode_file_name_component("a/b"), "a%2Fb");
        assert_eq!(encode_file_name_component("../x"), "..%2Fx");
        assert_eq!(encode_file_name_component("100%"), "100%25");
        assert_eq!(encode_file_name_component("a\nb\0c"), "a%0Ab%00c");
    }

    #[test]
    fn test_encoding_is_collision_free() {
        // "%" is encoded, so a literal "%2F" can't look like an encoded "/"
        assert_ne!(
            encode_file_name_component("a/b"),
            encode_file_name_component("a%2Fb")
        );
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("gu-split-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_values_with_unsafe_characters_get_encoded_file_names() {
        let dir = temp_dir("encode");
        process_file(
            std::io::Cursor::new("k,v\nx,1\na/b,2\n50%,3\n".as_bytes()),
            "k".to_string(),
            ',',
            ".csv".to_string(),
            &dir,
        )
        .unwrap();

        assert_eq!(
            fs::read_to_string(dir.join("k.x.csv")).unwrap(),
            "k,v\nx,1\n"
        );
        assert_eq!(
            fs::read_to_string(dir.join("k.a%2Fb.csv")).unwrap(),
            "k,v\na/b,2\n"
        );
        assert_eq!(
            fs::read_to_string(dir.join("k.50%25.csv")).unwrap(),
            "k,v\n50%,3\n"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_values_differing_only_by_case_are_an_error() {
        let dir = temp_dir("case");
        let err = process_file(
            std::io::Cursor::new("k,v\na,1\nA,2\n".as_bytes()),
            "k".to_string(),
            ',',
            ".csv".to_string(),
            &dir,
        )
        .expect_err("should fail")
        .to_string();
        assert!(
            err.contains("\"a\"") && err.contains("\"A\"") && err.contains("case"),
            "{err}"
        );

        // The rows read before the error are still written (a dropped writer flushes), but the
        // colliding value never gets a file, so it can't overwrite the first value's rows
        let files: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(files, vec!["k.a.csv"]);
        assert_eq!(
            fs::read_to_string(dir.join("k.a.csv")).unwrap(),
            "k,v\na,1\n"
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
