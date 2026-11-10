use flate2::read::MultiGzDecoder;
use gzp::{ZBuilder, ZWriter, deflate::Gzip};
use std::fs::File;
use std::io::{self, Cursor, Read, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, GuError>;

#[derive(Debug, Error)]
pub enum GuError {
    #[error(transparent)]
    IOError(#[from] io::Error),
    #[error(transparent)]
    ClapError(#[from] clap::Error),
    #[error(transparent)]
    CSVError(#[from] csv::Error),
    #[error(transparent)]
    RegexError(#[from] regex::Error),
    #[error(transparent)]
    JSONError(#[from] serde_json::Error),
    #[error(transparent)]
    ParseFloatError(#[from] std::num::ParseFloatError),
    #[error("{0}")]
    Message(String),
}

pub fn handle_broken_pipe(e: &GuError) {
    match e {
        GuError::IOError(io_err) => {
            if io_err.kind() == std::io::ErrorKind::BrokenPipe {
                std::process::exit(0);
            }
        }
        GuError::CSVError(e) => {
            if let csv::ErrorKind::Io(io_err) = e.kind()
                && io_err.kind() == std::io::ErrorKind::BrokenPipe
            {
                std::process::exit(0);
            }
        }
        _ => (),
    }
}

/// Delimited text to read, from a file or stdin. Gzip input is decompressed transparently.
pub struct Reader(Box<dyn Read>);

impl Read for Reader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

/// How much of the start of the input is examined to work out the delimiter
const SNIFF_SAMPLE_SIZE: usize = 8192;

impl Reader {
    /// Wrap any reader. If it starts with the gzip magic bytes it is decompressed
    /// (including multi-member files, e.g. bgzip output).
    pub fn new(inner: impl Read + 'static) -> Result<Reader> {
        let mut inner = inner;
        let mut magic = [0u8; 2];
        let n = read_up_to(&mut inner, &mut magic)?;
        let rejoined = Cursor::new(magic[..n].to_vec()).chain(inner);
        if n == GZIP_MAGIC.len() && magic == GZIP_MAGIC {
            Ok(Reader(Box::new(MultiGzDecoder::new(rejoined))))
        } else {
            Ok(Reader(Box::new(rejoined)))
        }
    }

    /// Work out the delimiter from the start of the input. The part of the input that was
    /// examined is put back, so the reader can be read from the beginning afterwards.
    pub fn sniff_csv_delimiter(&mut self) -> Result<char> {
        let mut sample = vec![0u8; SNIFF_SAMPLE_SIZE];
        let n = read_up_to(&mut self.0, &mut sample)?;
        sample.truncate(n);
        if sample.is_empty() {
            return Err(GuError::Message(
                "Unable to sniff delimiter from empty input".into(),
            ));
        }

        // If the sample filled the buffer, its last line was probably cut off part way through
        let analysed = match sample.iter().rposition(|&b| b == b'\n') {
            Some(last_newline) if n == SNIFF_SAMPLE_SIZE => &sample[..=last_newline],
            _ => &sample[..],
        };
        let delim = sniff_delimiter_from_sample(analysed)?;

        let rest = std::mem::replace(&mut self.0, Box::new(io::empty()));
        self.0 = Box::new(Cursor::new(sample).chain(rest));
        Ok(delim)
    }
}

/// Guess the delimiter from a sample of the start of a file.
///
/// A delimiter that appears the same (non-zero) number of times on every line, outside quotes,
/// is preferred: that is what a real delimiter looks like, whereas e.g. commas inside a
/// free-text column of a TSV file vary from line to line. If no candidate is that consistent
/// the one that appears most often overall is used, and if none appears at all the delimiter is
/// assumed to be a comma, so single-column files are still parsed correctly.
fn sniff_delimiter_from_sample(sample: &[u8]) -> Result<char> {
    let text = String::from_utf8_lossy(sample);
    let lines: Vec<&str> = text.lines().filter(|l| !l.is_empty()).take(5).collect();
    let candidates = [',', '\t', ';', '|', ' '];

    let mut best_consistent: Option<(char, usize)> = None;
    let mut best_total: (char, usize) = (',', 0);

    for &candidate in candidates.iter() {
        let counts: Vec<usize> = lines
            .iter()
            .map(|line| count_delimiter_outside_quotes(line, candidate))
            .collect();
        let total: usize = counts.iter().sum();
        if total == 0 {
            continue;
        }
        if total > best_total.1 {
            best_total = (candidate, total);
        }
        let consistent = counts.iter().all(|&c| c == counts[0]);
        if consistent && best_consistent.is_none_or(|(_, best)| total > best) {
            best_consistent = Some((candidate, total));
        }
    }

    Ok(best_consistent.map_or(best_total.0, |(c, _)| c))
}

fn count_delimiter_outside_quotes(line: &str, delimiter: char) -> usize {
    let mut count = 0;
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '"' {
            if in_quotes && chars.peek() == Some(&'"') {
                chars.next();
                continue;
            }
            in_quotes = !in_quotes;
            continue;
        }
        if !in_quotes && c == delimiter {
            count += 1;
        }
    }

    count
}

/// Canonical paths of every file opened for reading, so that `open_writer` can refuse to
/// overwrite a file that is being read from (which would silently destroy the input)
static INPUT_PATHS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

/// Open a file for reading. Gzip is detected from the file's magic bytes rather than its
/// extension, and multi-member gzip files (e.g. bgzip output) are read in full.
/// The name "stdin" reads from standard input, which is also decompressed if it is gzip.
pub fn open_reader(filename: &str) -> Result<Reader> {
    if filename == "stdin" {
        return Reader::new(std::io::stdin());
    }
    let f = File::open(filename)
        .map_err(|_| GuError::Message(format!("No such file or directory: {}", filename)))?;
    if let Ok(path) = std::fs::canonicalize(filename) {
        INPUT_PATHS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(path);
    }
    Reader::new(f)
}

/// Like `read_exact`, but returns the number of bytes read instead of failing at EOF
fn read_up_to<R: Read>(rdr: &mut R, buf: &mut [u8]) -> io::Result<usize> {
    let mut total = 0;
    while total < buf.len() {
        match rdr.read(&mut buf[total..]) {
            Ok(0) => break,
            Ok(n) => total += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(total)
}

/// Number of threads used to compress gzip output: the available cores, capped at 8
fn gzip_threads() -> usize {
    const MAX_GZIP_THREADS: usize = 8;
    std::thread::available_parallelism().map_or(1, |n| n.get().min(MAX_GZIP_THREADS))
}

enum OpenFile {
    Plain(File),
    Gzp(Box<dyn ZWriter<File>>),
}

/// A file that is only created (and truncated) when the first bytes are written to it.
/// Commands write nothing until their arguments and the input header have been validated,
/// so a failed run doesn't leave an empty or half-written output file behind.
pub struct LazyFile {
    path: String,
    file: Option<OpenFile>,
}

impl LazyFile {
    fn ensure_open(&mut self) -> io::Result<&mut OpenFile> {
        if self.file.is_none() {
            let f = File::create(&self.path)?;
            self.file = Some(if self.path.ends_with(".gz") {
                OpenFile::Gzp(
                    ZBuilder::<Gzip, _>::new()
                        .num_threads(gzip_threads())
                        .from_writer(f),
                )
            } else {
                OpenFile::Plain(f)
            });
        }
        Ok(self.file.as_mut().expect("file was just opened"))
    }
}

pub enum Writer {
    File(LazyFile),
    Stdout(std::io::Stdout),
}

impl Write for Writer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Writer::File(lazy) => {
                if buf.is_empty() {
                    return Ok(0);
                }
                match lazy.ensure_open()? {
                    OpenFile::Plain(f) => f.write(buf),
                    OpenFile::Gzp(gzp) => gzp.write(buf),
                }
            }
            Writer::Stdout(stdout) => stdout.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            // Nothing has been written, so there is nothing to flush (and no file to create)
            Writer::File(lazy) => match lazy.file.as_mut() {
                None => Ok(()),
                Some(OpenFile::Plain(f)) => f.flush(),
                Some(OpenFile::Gzp(gzp)) => gzp.flush(),
            },
            Writer::Stdout(stdout) => stdout.flush(),
        }
    }
}

/// Open a file for writing (gzipped if the name ends with .gz). The file is not created until
/// something is written to it. The name "stdout" writes to standard output.
/// Fails if the file is one that has already been opened for reading.
pub fn open_writer(filename: &str) -> Result<Writer> {
    if filename == "stdout" {
        let f = std::io::stdout();
        return Ok(Writer::Stdout(f));
    }
    if let Ok(path) = std::fs::canonicalize(filename)
        && INPUT_PATHS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(&path)
    {
        return Err(GuError::Message(format!(
            "Output file {} is also an input file; refusing to overwrite it",
            filename
        )));
    }
    Ok(Writer::File(LazyFile {
        path: filename.to_string(),
        file: None,
    }))
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_count_delimiter_outside_quotes() {
        assert_eq!(count_delimiter_outside_quotes("a,b,c", ','), 2);
        assert_eq!(count_delimiter_outside_quotes("a,\"b,c\",d", ','), 2);
        assert_eq!(count_delimiter_outside_quotes("\"a,b\",\"c,d\"", ','), 1);
        assert_eq!(count_delimiter_outside_quotes("\"a\"\"b\",c", ','), 1);
        assert_eq!(
            count_delimiter_outside_quotes("field1\tfield2\t\"x\tt\"\tfield3", '\t'),
            3
        );
    }

    #[test]
    fn test_sniff_delimiter_from_sample() {
        let comma_sample = b"id,name,score\n1,Alice,1.0\n2,Bob,2.0\n";
        assert_eq!(sniff_delimiter_from_sample(comma_sample).unwrap(), ',');

        let tab_sample = b"id\tname\tscore\n1\tAlice\t1.0\n2\tBob\t2.0\n";
        assert_eq!(sniff_delimiter_from_sample(tab_sample).unwrap(), '\t');

        let semicolon_sample = b"id;name;score\n1;\"A;lice\";1.0\n2;Bob;2.0\n";
        assert_eq!(sniff_delimiter_from_sample(semicolon_sample).unwrap(), ';');
    }

    #[test]
    fn test_sniff_prefers_a_consistent_delimiter() {
        // Commas in a free-text column vary from line to line; the tabs don't
        let tsv = b"id\tnote\tv\n1\ta, b, c, d\t3\n2\te, f\t4\n3\tg\t5\n";
        assert_eq!(sniff_delimiter_from_sample(tsv).unwrap(), '\t');

        // And the same the other way round: spaces in a free-text column of a CSV
        let csv = b"id,note,v\n1,hello big wide world,3\n2,a b,4\n3,c,5\n";
        assert_eq!(sniff_delimiter_from_sample(csv).unwrap(), ',');
    }

    #[test]
    fn test_sniff_falls_back_to_the_most_common_delimiter() {
        // No delimiter is consistent (ragged rows), so use the one that appears most
        let ragged = b"a;b;c\n1;2\n3;4;5;6\n";
        assert_eq!(sniff_delimiter_from_sample(ragged).unwrap(), ';');
    }

    #[test]
    fn test_sniff_header_only_and_single_column() {
        assert_eq!(sniff_delimiter_from_sample(b"a\tb\tc\n").unwrap(), '\t');
        assert_eq!(sniff_delimiter_from_sample(b"id\n1\n2\n").unwrap(), ',');
        // Blank lines don't count as lines with no delimiters
        assert_eq!(sniff_delimiter_from_sample(b"a;b\n\n1;2\n").unwrap(), ';');
    }

    /// A reader that returns a single byte per `read`, like a slow pipe can
    struct OneByteAtATime<R>(R);

    impl<R: Read> Read for OneByteAtATime<R> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let n = buf.len().min(1);
            self.0.read(&mut buf[..n])
        }
    }

    fn read_all(mut rdr: impl Read) -> String {
        let mut s = String::new();
        rdr.read_to_string(&mut s).unwrap();
        s
    }

    #[test]
    fn test_sniff_copes_with_short_reads() {
        let input = "id\tnote\na\tx\nb\ty\n";
        let mut rdr = Reader::new(OneByteAtATime(Cursor::new(input.as_bytes().to_vec()))).unwrap();
        assert_eq!(rdr.sniff_csv_delimiter().unwrap(), '\t');
        assert_eq!(read_all(rdr), input);
    }

    #[test]
    fn test_sniffing_does_not_consume_or_lose_input() {
        let input = "id,v\n1,2\n3,4\n";
        let mut rdr = Reader::new(Cursor::new(input.as_bytes().to_vec())).unwrap();
        assert_eq!(rdr.sniff_csv_delimiter().unwrap(), ',');
        // Sniffing again (previously this dropped the rest of the stream) gives the same answer
        assert_eq!(rdr.sniff_csv_delimiter().unwrap(), ',');
        assert_eq!(read_all(rdr), input);
    }

    #[test]
    fn test_sniff_ignores_a_line_cut_off_by_the_end_of_the_sample() {
        // Five long lines, each with two tabs (the second at the end) and a different number of
        // commas. The sample ends part way through a line, which would make the tab count
        // inconsistent if that last partial line weren't dropped.
        let mut input = String::new();
        for i in 0..5 {
            input.push_str("id\t");
            input.push_str(&"a,".repeat(1000 + 100 * i));
            input.push_str("\tend\n");
        }
        assert!(input.len() > SNIFF_SAMPLE_SIZE);
        let mut rdr = Reader::new(Cursor::new(input.clone().into_bytes())).unwrap();
        assert_eq!(rdr.sniff_csv_delimiter().unwrap(), '\t');
        assert_eq!(read_all(rdr), input);
    }

    #[test]
    fn test_sniff_empty_input_is_an_error() {
        let mut rdr = Reader::new(Cursor::new(Vec::new())).unwrap();
        assert!(rdr.sniff_csv_delimiter().is_err());
    }

    fn gzip_bytes(data: &str) -> Vec<u8> {
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(data.as_bytes()).unwrap();
        enc.finish().unwrap()
    }

    #[test]
    fn test_reader_decompresses_gzip_including_multiple_members_and_sniffs_it() {
        let mut bytes = gzip_bytes("id\tv\n1\t2\n");
        bytes.extend(gzip_bytes("3\t4\n"));
        let mut rdr = Reader::new(OneByteAtATime(Cursor::new(bytes))).unwrap();
        assert_eq!(rdr.sniff_csv_delimiter().unwrap(), '\t');
        assert_eq!(read_all(rdr), "id\tv\n1\t2\n3\t4\n");
    }

    #[test]
    fn test_reader_passes_through_short_and_non_gzip_input() {
        assert_eq!(
            read_all(Reader::new(Cursor::new(b"".to_vec())).unwrap()),
            ""
        );
        assert_eq!(
            read_all(Reader::new(Cursor::new(b"x".to_vec())).unwrap()),
            "x"
        );
        assert_eq!(
            read_all(Reader::new(Cursor::new(b"id,v\n".to_vec())).unwrap()),
            "id,v\n"
        );
    }
}
