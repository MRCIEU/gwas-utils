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
