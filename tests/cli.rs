//! End-to-end tests that run the real `gu` binary.

use flate2::Compression;
use flate2::read::MultiGzDecoder;
use flate2::write::GzEncoder;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A fresh, empty scratch directory that is removed when dropped
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "gu-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let p = self.path(name);
        fs::write(&p, contents).unwrap();
        p
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn gu(args: &[&dyn AsRef<std::ffi::OsStr>]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_gu"))
        .args(args.iter().map(|a| a.as_ref()))
        .output()
        .unwrap()
}

/// Run `gu` with `dir` as its working directory
fn gu_in(dir: &Path, args: &[&dyn AsRef<std::ffi::OsStr>]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_gu"))
        .current_dir(dir)
        .args(args.iter().map(|a| a.as_ref()))
        .output()
        .unwrap()
}

fn stdout(o: &Output) -> String {
    String::from_utf8(o.stdout.clone()).unwrap()
}

fn stderr(o: &Output) -> String {
    String::from_utf8(o.stderr.clone()).unwrap()
}

fn gzip(data: &str) -> Vec<u8> {
    let mut enc = GzEncoder::new(Vec::new(), Compression::default());
    enc.write_all(data.as_bytes()).unwrap();
    enc.finish().unwrap()
}

fn gunzip_all(path: &Path) -> String {
    let mut s = String::new();
    MultiGzDecoder::new(fs::File::open(path).unwrap())
        .read_to_string(&mut s)
        .unwrap();
    s
}

const SUMSTATS: &str = "ID,P,BETA,SE\na,0.01,1,0.5\nb,NA,NA,NA\nc,0.5,2,1\n";

// --- gzip handling ---

#[test]
fn multi_member_gzip_is_read_in_full() {
    // bgzip output is a series of concatenated gzip members
    let dir = TempDir::new();
    let mut bytes = gzip("x,y\n1,1\n2,2\n3,3\n");
    bytes.extend(gzip("4,4\n5,5\n6,6\n"));
    let input = dir.path("multi.csv.gz");
    fs::write(&input, bytes).unwrap();

    let out = gu(&[&"csv", &"select", &input, &"-c", &"x"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "x\n1\n2\n3\n4\n5\n6\n");
}

#[test]
fn gzip_is_detected_by_content_not_extension() {
    let dir = TempDir::new();
    let gz_named_csv = dir.path("actually_gzipped.csv");
    fs::write(&gz_named_csv, gzip(SUMSTATS)).unwrap();
    let plain_named_gz = dir.write("actually_plain.csv.gz", SUMSTATS);

    for input in [&gz_named_csv, &plain_named_gz] {
        let out = gu(&[&"csv", &"select", input, &"-c", &"ID"]);
        assert!(out.status.success(), "{}", stderr(&out));
        assert_eq!(stdout(&out), "ID\na\nb\nc\n");
    }
}

#[test]
fn gzip_output_round_trips() {
    let dir = TempDir::new();
    let input = dir.write("in.csv", SUMSTATS);
    let output = dir.path("out.csv.gz");

    let out = gu(&[&"csv", &"delim", &input, &"-d", &"\\t", &"-o", &output]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        gunzip_all(&output),
        "ID\tP\tBETA\tSE\na\t0.01\t1\t0.5\nb\tNA\tNA\tNA\nc\t0.5\t2\t1\n"
    );
}

#[test]
fn reads_from_stdin_and_sniffs_delimiter() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_gu"))
        .args(["csv", "select", "-c", "ID", "BETA"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(SUMSTATS.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    assert_eq!(stdout(&out), "ID,BETA\na,1\nb,NA\nc,2\n");
}

// --- output file safety ---

#[test]
fn refuses_to_overwrite_its_own_input() {
    let dir = TempDir::new();
    let input = dir.write("data.csv", SUMSTATS);

    let out = gu(&[&"csv", &"select", &input, &"-c", &"ID", &"-o", &input]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("also an input file"),
        "{}",
        stderr(&out)
    );
    assert_eq!(fs::read_to_string(&input).unwrap(), SUMSTATS);
}

#[test]
fn refuses_to_overwrite_its_own_input_via_another_path() {
    let dir = TempDir::new();
    let input = dir.write("data.csv", SUMSTATS);
    let alias = dir.0.join(".").join("data.csv");

    let out = gu(&[&"csv", &"select", &input, &"-c", &"ID", &"-o", &alias]);
    assert!(!out.status.success());
    assert_eq!(fs::read_to_string(&input).unwrap(), SUMSTATS);
}

#[test]
fn failed_run_does_not_create_output_file() {
    let dir = TempDir::new();
    let input = dir.write("data.csv", SUMSTATS);
    let output = dir.path("out.csv");

    let out = gu(&[&"csv", &"select", &input, &"-c", &"NOPE", &"-o", &output]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("NOPE"));
    assert!(!output.exists(), "output file should not have been created");
}

#[test]
fn failed_run_does_not_create_gzip_output_file() {
    let dir = TempDir::new();
    let input = dir.write("data.csv", SUMSTATS);
    let output = dir.path("out.csv.gz");

    let out = gu(&[&"csv", &"addz", &input, &"--beta", &"NOPE", &"-o", &output]);
    assert!(!out.status.success());
    assert!(!output.exists(), "output file should not have been created");
}

// --- select ---

#[test]
fn select_index_zero_is_an_error_not_a_panic() {
    let dir = TempDir::new();
    let input = dir.write("data.csv", SUMSTATS);

    let out = gu(&[&"csv", &"select", &input, &"-i", &"0"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(!stderr(&out).contains("panicked"), "{}", stderr(&out));
}

// --- missing values ---

#[test]
fn filter_skips_missing_values_in_numeric_comparisons() {
    let dir = TempDir::new();
    let input = dir.write("data.csv", SUMSTATS);

    let out = gu(&[&"csv", &"filter", &input, &"-e", &"P < 0.05"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "ID,P,BETA,SE\na,0.01,1,0.5\n");
}

#[test]
fn filter_reports_invalid_values_with_column_and_line() {
    let dir = TempDir::new();
    let input = dir.write("data.csv", "ID,P\na,0.01\nb,oops\n");

    let out = gu(&[&"csv", &"filter", &input, &"-e", &"P < 0.05"]);
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(
        err.contains("\"P\"") && err.contains("line 3") && err.contains("oops"),
        "{err}"
    );
}

#[test]
fn addz_writes_na_for_missing_values() {
    let dir = TempDir::new();
    let input = dir.write("data.csv", SUMSTATS);

    let out = gu(&[&"csv", &"addz", &input]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        stdout(&out),
        "ID,P,BETA,SE,Z\na,0.01,1,0.5,2.000000\nb,NA,NA,NA,NA\nc,0.5,2,1,2.000000\n"
    );
}

#[test]
fn addz_reports_invalid_values_with_column_and_line() {
    let dir = TempDir::new();
    let input = dir.write("data.csv", "BETA,SE\n1,2\nfoo,2\n");

    let out = gu(&[&"csv", &"addz", &input]);
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(
        err.contains("\"BETA\"") && err.contains("line 3") && err.contains("foo"),
        "{err}"
    );
}

#[test]
fn addp_writes_na_for_missing_but_errors_on_garbage() {
    let dir = TempDir::new();
    let good = dir.write("good.csv", "LOG10P\n2\nNA\n");
    let bad = dir.write("bad.csv", "LOG10P\n2\nfoo\n");

    let out = gu(&[&"csv", &"addp", &good, &"-d", &","]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "LOG10P,P\n2,1E-2\nNA,NA\n");

    let out = gu(&[&"csv", &"addp", &bad, &"-d", &","]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("foo"));
}

#[test]
fn lambda_skips_missing_values_but_errors_on_garbage() {
    let dir = TempDir::new();
    let good = dir.write("good.csv", "P\n0.5\nNA\n0.5\n");
    let bad = dir.write("bad.csv", "P\n0.5\nfoo\n");

    let out = gu(&[&"csv", &"stats", &"lambda", &good, &"-d", &","]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "1.000000\n");

    let out = gu(&[&"csv", &"stats", &"lambda", &bad, &"-d", &","]);
    assert!(!out.status.success());
}

// --- merge ---

#[test]
fn merge_errors_on_duplicate_keys_in_first_file_by_default() {
    let dir = TempDir::new();
    let f1 = dir.write("f1.csv", "ID,A\nx,1\nx,2\n");
    let f2 = dir.write("f2.csv", "ID,B\nx,9\n");
    let output = dir.path("out.csv");

    let out = gu(&[&"csv", &"merge", &f1, &f2, &"-c", &"ID", &"-o", &output]);
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(
        err.contains("Duplicate key \"x\"") && err.contains("--on-duplicate"),
        "{err}"
    );
    assert!(!output.exists(), "output file should not have been created");
}

#[test]
fn merge_on_duplicate_first_and_last() {
    let dir = TempDir::new();
    let f1 = dir.write("f1.csv", "ID,A\nx,1\nx,2\n");
    let f2 = dir.write("f2.csv", "ID,B\nx,9\n");

    let out = gu(&[
        &"csv",
        &"merge",
        &f1,
        &f2,
        &"-c",
        &"ID",
        &"--on-duplicate",
        &"first",
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "ID,A,B\nx,1,9\n");

    let out = gu(&[
        &"csv",
        &"merge",
        &f1,
        &f2,
        &"-c",
        &"ID",
        &"--on-duplicate",
        &"last",
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "ID,A,B\nx,2,9\n");
}

#[test]
fn merge_repeated_keys_in_second_file_expand_unless_strict() {
    let dir = TempDir::new();
    let f1 = dir.write("f1.csv", "ID,A\nx,1\n");
    let f2 = dir.write("f2.csv", "ID,B\nx,8\nx,9\n");

    let out = gu(&[&"csv", &"merge", &f1, &f2, &"-c", &"ID"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "ID,A,B\nx,1,8\nx,1,9\n");

    // --strict checks the second file too
    let out = gu(&[&"csv", &"merge", &f1, &f2, &"-c", &"ID", &"--strict"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("second file"), "{}", stderr(&out));

    let out = gu(&[
        &"csv",
        &"merge",
        &f1,
        &f2,
        &"-c",
        &"ID",
        &"--strict",
        &"--on-duplicate",
        &"last",
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "ID,A,B\nx,1,9\n");
}

#[test]
fn merge_works_on_gzipped_inputs_and_outputs() {
    let dir = TempDir::new();
    let f1 = dir.path("f1.csv.gz");
    fs::write(&f1, gzip("ID,A\nx,1\ny,2\n")).unwrap();
    let f2 = dir.write("f2.csv", "ID,B\ny,9\nz,8\n");
    let output = dir.path("out.csv.gz");

    let out = gu(&[
        &"csv", &"merge", &f1, &f2, &"-c", &"ID", &"-j", &"outer", &"-f", &"NA", &"-o", &output,
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(gunzip_all(&output), "ID,A,B\nx,1,NA\ny,2,9\nz,NA,8\n");
}

// --- delimiter auto-detection ---

#[test]
fn sniffs_each_supported_delimiter() {
    let dir = TempDir::new();
    for (name, delim) in [
        ("comma", ','),
        ("tab", '\t'),
        ("semicolon", ';'),
        ("pipe", '|'),
        ("space", ' '),
    ] {
        let contents = format!("ID{d}P{d}BETA\na{d}0.1{d}1\nb{d}0.2{d}2\n", d = delim);
        let input = dir.write(&format!("{name}.txt"), &contents);

        // Selecting every column should reproduce the file unchanged, delimiter included
        let out = gu(&[&"csv", &"select", &input, &"-c", &"ID", &"P", &"BETA"]);
        assert!(out.status.success(), "{name}: {}", stderr(&out));
        assert_eq!(stdout(&out), contents, "{name}");
    }
}

#[test]
fn sniffing_ignores_delimiters_inside_quotes() {
    let dir = TempDir::new();
    let input = dir.write(
        "quoted.tsv",
        "ID\tNOTE\tVAL\na\t\"x, y, z, w\"\t1\nb\t\"p, q, r, s\"\t2\n",
    );

    let out = gu(&[&"csv", &"select", &input, &"-c", &"ID", &"VAL"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "ID\tVAL\na\t1\nb\t2\n");
}

#[test]
fn single_column_file_defaults_to_comma() {
    let dir = TempDir::new();
    let input = dir.write("one.csv", "ID\na\nb\n");

    let out = gu(&[&"csv", &"select", &input, &"-c", &"ID"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "ID\na\nb\n");
}

#[test]
fn sniffs_delimiter_of_gzipped_files() {
    // Sniffing has to rewind the decompressor, which is separate code from the plain-file path
    let dir = TempDir::new();
    let input = dir.path("tabs.tsv.gz");
    fs::write(&input, gzip("ID\tP\na\t0.1\nb\t0.2\n")).unwrap();

    let out = gu(&[&"csv", &"select", &input, &"-c", &"P"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "P\n0.1\n0.2\n");
}

#[test]
fn explicit_delimiter_overrides_sniffing() {
    let dir = TempDir::new();
    // Sniffing alone would pick comma (it's the most common character), but the file really is
    // tab separated
    let input = dir.write("odd.tsv", "ID\tNOTE\na\t1,2,3,4\nb\t5,6,7,8\n");

    let out = gu(&[&"csv", &"select", &input, &"-c", &"NOTE", &"-d", &"\\t"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "NOTE\n1,2,3,4\n5,6,7,8\n");
}

#[test]
fn empty_input_is_an_error() {
    let dir = TempDir::new();
    let input = dir.write("empty.csv", "");
    let out = gu(&[&"csv", &"select", &input, &"-c", &"ID"]);
    assert!(!out.status.success());
}

// --- select ---

#[test]
fn select_by_regex_index_and_no_reorder() {
    let dir = TempDir::new();
    let input = dir.write("data.csv", SUMSTATS);

    let out = gu(&[&"csv", &"select", &input, &"-r", &"^B", &"^ID$"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "BETA,ID\n1,a\nNA,b\n2,c\n");

    let out = gu(&[
        &"csv",
        &"select",
        &input,
        &"-r",
        &"^B",
        &"^ID$",
        &"--no-reorder",
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "ID,BETA\na,1\nb,NA\nc,2\n");

    let out = gu(&[&"csv", &"select", &input, &"-i", &"4", &"1"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "SE,ID\n0.5,a\nNA,b\n1,c\n");

    let out = gu(&[&"csv", &"select", &input, &"-i", &"5"]);
    assert!(!out.status.success());
    let out = gu(&[&"csv", &"select", &input]);
    assert!(!out.status.success());
}

// --- filter ---

#[test]
fn filter_by_regex_invert_and_any() {
    let dir = TempDir::new();
    let input = dir.write(
        "alleles.csv",
        "ID,A1,POS\na,A,10\nb,AC,20\nc,G,30\nd,TT,40\n",
    );

    let out = gu(&[&"csv", &"filter", &input, &"-c", &"A1", &"-r", &"^[ACGT]$"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "ID,A1,POS\na,A,10\nc,G,30\n");

    let out = gu(&[
        &"csv",
        &"filter",
        &input,
        &"-c",
        &"A1",
        &"-r",
        &"^[ACGT]$",
        &"-v",
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "ID,A1,POS\nb,AC,20\nd,TT,40\n");

    let out = gu(&[
        &"csv",
        &"filter",
        &input,
        &"-e",
        &"POS < 15",
        &"A1 == G",
        &"--any",
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "ID,A1,POS\na,A,10\nc,G,30\n");

    // Without --any, every expression must hold
    let out = gu(&[&"csv", &"filter", &input, &"-e", &"POS < 35", &"A1 == G"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "ID,A1,POS\nc,G,30\n");
}

#[test]
fn filter_with_unknown_column_fails_before_writing_anything() {
    let dir = TempDir::new();
    let input = dir.write("data.csv", SUMSTATS);
    let out = gu(&[&"csv", &"filter", &input, &"-e", &"NOPE > 1"]);
    assert!(!out.status.success());
    assert_eq!(stdout(&out), "");
}

// --- cat ---

#[test]
fn cat_concatenates_files_and_writes_one_header() {
    let dir = TempDir::new();
    let f1 = dir.write("f1.csv", "ID,V\na,1\n");
    let f2 = dir.write("f2.csv", "ID,V\nb,2\n");
    let f3 = dir.path("f3.csv.gz");
    fs::write(&f3, gzip("ID,V\nc,3\n")).unwrap();

    let out = gu(&[&"csv", &"cat", &f1, &f2, &f3]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "ID,V\na,1\nb,2\nc,3\n");
}

#[test]
fn cat_rejects_mismatched_headers_and_inconsistent_delimiters() {
    let dir = TempDir::new();
    let f1 = dir.write("f1.csv", "ID,V\na,1\n");
    let different_header = dir.write("f2.csv", "ID,W\nb,2\n");
    let different_delim = dir.write("f3.tsv", "ID\tV\nb\t2\n");

    let out = gu(&[&"csv", &"cat", &f1, &different_header]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("Mismatched headers"),
        "{}",
        stderr(&out)
    );

    let out = gu(&[&"csv", &"cat", &f1, &different_delim]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("Inconsistent delimiters"),
        "{}",
        stderr(&out)
    );
}

// --- split ---

#[test]
fn split_writes_one_file_per_value_into_the_working_directory() {
    let dir = TempDir::new();
    let input = dir.write("data.csv", "CHR,POS\n1,10\n2,20\n1,30\n");

    let out = gu_in(&dir.0, &[&"csv", &"split", &"data.csv", &"-c", &"CHR"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        fs::read_to_string(dir.path("CHR.1.csv")).unwrap(),
        "CHR,POS\n1,10\n1,30\n"
    );
    assert_eq!(
        fs::read_to_string(dir.path("CHR.2.csv")).unwrap(),
        "CHR,POS\n2,20\n"
    );
    assert_eq!(
        fs::read_to_string(&input).unwrap(),
        "CHR,POS\n1,10\n2,20\n1,30\n"
    );
}

#[test]
fn split_with_custom_and_gzip_suffix() {
    let dir = TempDir::new();
    dir.write("data.csv", "CHR,POS\n1,10\n2,20\n");

    let out = gu_in(
        &dir.0,
        &[
            &"csv",
            &"split",
            &"data.csv",
            &"-c",
            &"CHR",
            &"-s",
            &"tsv.gz",
        ],
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(gunzip_all(&dir.path("CHR.1.tsv.gz")), "CHR,POS\n1,10\n");
    assert_eq!(gunzip_all(&dir.path("CHR.2.tsv.gz")), "CHR,POS\n2,20\n");
}

#[test]
fn split_on_unknown_column_creates_no_files() {
    let dir = TempDir::new();
    dir.write("data.csv", "CHR,POS\n1,10\n");

    let out = gu_in(&dir.0, &[&"csv", &"split", &"data.csv", &"-c", &"NOPE"]);
    assert!(!out.status.success());
    let mut files: Vec<_> = fs::read_dir(&dir.0)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    files.sort();
    assert_eq!(files, vec!["data.csv"]);
}

// --- reheader ---

#[test]
fn reheader_with_a_new_list_or_renamed_columns() {
    let dir = TempDir::new();
    let input = dir.write("data.csv", "A,B,C\n1,2,3\n");

    let out = gu(&[&"csv", &"reheader", &input, &"-l", &"X", &"Y", &"Z"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "X,Y,Z\n1,2,3\n");

    let out = gu(&[&"csv", &"reheader", &input, &"-c", &"B=BETA", &"C=SE"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "A,BETA,SE\n1,2,3\n");
}

#[test]
fn reheader_rejects_wrong_length_list_and_bad_rename() {
    let dir = TempDir::new();
    let input = dir.write("data.csv", "A,B,C\n1,2,3\n");

    let out = gu(&[&"csv", &"reheader", &input, &"-l", &"X", &"Y"]);
    assert!(!out.status.success());
    let out = gu(&[&"csv", &"reheader", &input, &"-c", &"NOPE=X"]);
    assert!(!out.status.success());
    let out = gu(&[&"csv", &"reheader", &input, &"-c", &"A:X"]);
    assert!(!out.status.success());
}

// --- delim ---

#[test]
fn delim_quotes_fields_that_contain_the_new_delimiter() {
    let dir = TempDir::new();
    let input = dir.write("data.tsv", "ID\tNOTE\na\tx,y\n");

    let out = gu(&[&"csv", &"delim", &input, &"-d", &","]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "ID,NOTE\na,\"x,y\"\n");
}

// --- regenify (hidden subcommand) ---

#[test]
fn regenify_writes_tsv_with_missing_values_as_na() {
    let dir = TempDir::new();
    let input = dir.write("data.csv", "ID,V\na,-1\nb,\nc,3\n");

    let out = gu(&[&"csv", &"regenify", &input]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "ID\tV\na\tNA\nb\tNA\nc\t3\n");

    let out = gu(&[&"csv", &"regenify", &input, &"--no-rep-neg-one"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "ID\tV\na\t-1\nb\tNA\nc\t3\n");
}

// --- dn make_dxfuse_manifest ---

#[test]
fn make_dxfuse_manifest_extracts_file_ids() {
    let dir = TempDir::new();
    let output = dir.path("manifest.json");

    let out = gu(&[
        &"dn",
        &"make_dxfuse_manifest",
        &"-f",
        &"file-FxY5660JkF6BB3Jq9680pjq5",
        &"{\"$dnanexus_link\": \"file-J5Q57p8JX3J2JBJ6fPqfq4bO\"}",
        &"-p",
        &"project-xxxx",
        &"-o",
        &output,
    ]);
    assert!(out.status.success(), "{}", stderr(&out));

    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&output).unwrap()).unwrap();
    let files = manifest["Files"].as_array().unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0]["file_id"], "file-FxY5660JkF6BB3Jq9680pjq5");
    assert_eq!(files[1]["file_id"], "file-J5Q57p8JX3J2JBJ6fPqfq4bO");
    assert_eq!(files[1]["proj_id"], "project-xxxx");
    assert_eq!(files[1]["parent"], "/");
}

// --- top-level flags and usage errors ---

#[test]
fn version_licence_and_help_flags() {
    let out = gu(&[&"--version"]);
    assert!(out.status.success());
    assert_eq!(
        stdout(&out).trim(),
        format!("gu {}", env!("CARGO_PKG_VERSION"))
    );

    let out = gu(&[&"-l"]);
    assert!(out.status.success());
    assert!(stdout(&out).starts_with("gwas-utils is licensed under the MIT License"));

    let out = gu(&[&"--licenses"]);
    assert!(out.status.success());
    assert!(stdout(&out).len() > gu(&[&"-l"]).stdout.len());

    // The hidden subcommand stays out of the help listing
    let out = gu(&[&"csv", &"--help"]);
    assert!(out.status.success());
    assert!(!stdout(&out).contains("regenify"));
}

#[test]
fn usage_errors_exit_non_zero() {
    assert!(!gu(&[&"nonsense"]).status.success());
    assert!(!gu(&[&"csv", &"select"]).status.success());
    let out = gu(&[&"csv", &"select", &"/no/such/file.csv", &"-c", &"ID"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("No such file"), "{}", stderr(&out));
}

#[test]
fn broken_pipe_exits_quietly() {
    // e.g. `gu csv delim big.csv -d '\t' | head -n1` must not print an error or panic
    let dir = TempDir::new();
    let mut contents = String::from("ID,V\n");
    for i in 0..200_000 {
        contents.push_str(&format!("{i},{i}\n"));
    }
    let input = dir.write("big.csv", &contents);

    let mut child = Command::new(env!("CARGO_BIN_EXE_gu"))
        .args(["csv", "delim"])
        .arg(&input)
        .args(["-d", "\\t"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Read a little and close the pipe while gu is still writing
    let mut stdout_pipe = child.stdout.take().unwrap();
    let mut first = [0u8; 16];
    stdout_pipe.read_exact(&mut first).unwrap();
    drop(stdout_pipe);
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "status {:?}: {}",
        out.status,
        stderr(&out)
    );
    assert_eq!(stderr(&out), "");
}

// --- stdin ---

/// Run `gu` with `args`, writing each chunk of `chunks` to its stdin with a pause in between
fn gu_stdin_chunks(args: &[&str], chunks: &[&[u8]]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_gu"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for (i, chunk) in chunks.iter().enumerate() {
        if i > 0 {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        stdin.write_all(chunk).unwrap();
        stdin.flush().unwrap();
    }
    drop(stdin);
    child.wait_with_output().unwrap()
}

#[test]
fn stdin_gzip_is_decompressed() {
    let out = gu_stdin_chunks(
        &["csv", "select", "-c", "ID"],
        &[&gzip("ID\tP\na\t0.1\nb\t0.2\n")],
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "ID\na\nb\n");
}

#[test]
fn stdin_multi_member_gzip_is_decompressed() {
    let mut bytes = gzip("x,y\n1,1\n");
    bytes.extend(gzip("2,2\n"));
    let out = gu_stdin_chunks(&["csv", "select", "-c", "x"], &[&bytes]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "x\n1\n2\n");
}

#[test]
fn stdin_arriving_in_small_slow_chunks_is_still_sniffed_correctly() {
    // The first chunk contains no delimiter at all, so sniffing must wait for more input
    let out = gu_stdin_chunks(
        &["csv", "select", "-c", "P"],
        &[b"ID", b"\tP\na\t1\n", b"b\t2\n"],
    );
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "P\n1\n2\n");
}

#[test]
fn tsv_with_commas_in_a_text_column_is_read_as_tsv() {
    let dir = TempDir::new();
    let input = dir.write(
        "notes.tsv",
        "ID\tNOTE\tV\na\tone, two, three, four, five, six, seven\t1\nb\tfive, six\t2\nc\tsix, seven, eight, nine, ten\t3\n",
    );

    let out = gu(&[&"csv", &"select", &input, &"-c", &"ID", &"V"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "ID\tV\na\t1\nb\t2\nc\t3\n");
}
