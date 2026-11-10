use flate2::read::MultiGzDecoder;
use gzp::{ZBuilder, ZWriter, deflate::Gzip};
use std::fs::File;
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
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

pub enum Reader {
    File(std::fs::File),
    GzFile(MultiGzDecoder<std::fs::File>),
    Stdin(std::io::Stdin),
    Buffered(Box<dyn Read>),
}

impl Read for Reader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Reader::File(f) => f.read(buf),
            Reader::GzFile(gz) => gz.read(buf),
            Reader::Stdin(stdin) => stdin.read(buf),
            Reader::Buffered(inner) => inner.read(buf),
        }
    }
}

impl Reader {
    pub fn sniff_csv_delimiter(&mut self) -> Result<char> {
        let mut sample = vec![0u8; 8192];
        let n = match self {
            Reader::File(f) => f.read(&mut sample)?,
            Reader::GzFile(gz) => gz.read(&mut sample)?,
            Reader::Stdin(stdin) => stdin.read(&mut sample)?,
            Reader::Buffered(inner) => inner.read(&mut sample)?,
        };
        sample.truncate(n);
        if sample.is_empty() {
            return Err(GuError::Message(
                "Unable to sniff delimiter from empty input".into(),
            ));
        }

        let delim = sniff_delimiter_from_sample(&sample)?;

        match self {
            Reader::File(f) => {
                f.seek(SeekFrom::Start(0))?;
            }
            Reader::GzFile(gz) => {
                let inner_file = gz.get_mut();
                inner_file.seek(SeekFrom::Start(0))?;
                *gz = MultiGzDecoder::new(inner_file.try_clone()?);
            }
            Reader::Stdin(_) => {
                let cursor = Cursor::new(sample);
                let chain = cursor.chain(std::io::stdin());
                *self = Reader::Buffered(Box::new(chain));
            }
            Reader::Buffered(_) => {
                let cursor = Cursor::new(sample);
                *self = Reader::Buffered(Box::new(cursor));
            }
        }

        Ok(delim)
    }
}

fn sniff_delimiter_from_sample(sample: &[u8]) -> Result<char> {
    let text = String::from_utf8_lossy(sample);
    let candidates = [',', '\t', ';', '|', ' '];
    let mut best: (char, usize) = (',', 0); // defaulting to comma if no better delimiter found means single-column csv files are still parsed correctly

    for &candidate in candidates.iter() {
        let mut count = 0;
        for line in text.lines().take(5) {
            count += count_delimiter_outside_quotes(line, candidate);
        }
        if count == 0 {
            continue;
        }
        if count > best.1 {
            best = (candidate, count);
        }
    }

    Ok(best.0)
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
/// The name "stdin" reads from standard input (which is not decompressed).
pub fn open_reader(filename: &str) -> Result<Reader> {
    if filename == "stdin" {
        let f = std::io::stdin();
        return Ok(Reader::Stdin(f));
    }
    let mut f = File::open(filename)
        .map_err(|_| GuError::Message(format!("No such file or directory: {}", filename)))?;
    if let Ok(path) = std::fs::canonicalize(filename) {
        INPUT_PATHS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(path);
    }

    let mut magic = [0u8; 2];
    let n = read_up_to(&mut f, &mut magic)?;
    f.seek(SeekFrom::Start(0))?;
    if n == GZIP_MAGIC.len() && magic == GZIP_MAGIC {
        Ok(Reader::GzFile(MultiGzDecoder::new(f)))
    } else {
        Ok(Reader::File(f))
    }
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
                OpenFile::Gzp(ZBuilder::<Gzip, _>::new().num_threads(8).from_writer(f))
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
}
