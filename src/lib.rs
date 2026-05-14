use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use std::fs::File;
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
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
    GzFile(GzDecoder<std::fs::File>),
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
    pub fn sniff(&mut self) -> Result<char> {
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
                *gz = GzDecoder::new(inner_file.try_clone()?);
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
    let mut best: Option<(char, usize)> = None;

    for &candidate in candidates.iter() {
        let mut count = 0;
        for line in text.lines().take(5) {
            count += count_delimiter_outside_quotes(line, candidate);
        }
        if count == 0 {
            continue;
        }
        if let Some((_, best_count)) = best {
            if count > best_count {
                best = Some((candidate, count));
            }
        } else {
            best = Some((candidate, count));
        }
    }

    best.map(|(d, _)| d).ok_or(GuError::Message(
        "Unable to detect delimiter from input sample".into(),
    ))
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

pub fn open_reader(filename: &str) -> Result<Reader> {
    if filename == "stdin" {
        let f = std::io::stdin();
        return Ok(Reader::Stdin(f));
    }
    let f = File::open(filename)?;
    if filename.ends_with(".gz") {
        let gz = GzDecoder::new(f);
        Ok(Reader::GzFile(gz))
    } else {
        Ok(Reader::File(f))
    }
}

pub enum Writer {
    File(std::fs::File),
    GzFile(GzEncoder<std::fs::File>),
    Stdout(std::io::Stdout),
}

impl Write for Writer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Writer::File(f) => f.write(buf),
            Writer::GzFile(gz) => gz.write(buf),
            Writer::Stdout(stdout) => stdout.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Writer::File(f) => f.flush(),
            Writer::GzFile(gz) => gz.flush(),
            Writer::Stdout(stdout) => stdout.flush(),
        }
    }
}

pub fn open_writer(filename: &str) -> Result<Writer> {
    if filename == "stdout" {
        let f = std::io::stdout();
        return Ok(Writer::Stdout(f));
    }
    let f = File::create(filename)?;
    if filename.ends_with(".gz") {
        let gz = GzEncoder::new(f, Compression::default());
        Ok(Writer::GzFile(gz))
    } else {
        Ok(Writer::File(f))
    }
}

pub fn get_delimeter_from_cli_argument(sep: &str) -> Result<char> {
    let single_ascii_err = "Delimiter must be a single ASCII character".to_string();
    let c = match sep {
        "\\t" => '\t',
        s if s.chars().count() == 1 => s.chars().next().unwrap(),
        _ => return Err(GuError::Message(single_ascii_err)),
    };
    if !c.is_ascii() {
        return Err(GuError::Message(single_ascii_err));
    }
    Ok(c)
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_get_delimiter() {
        assert_eq!(get_delimeter_from_cli_argument("\t").unwrap(), '\t');
        assert_eq!(get_delimeter_from_cli_argument("\\t").unwrap(), '\t');
        assert_eq!(get_delimeter_from_cli_argument(r#"	"#).unwrap(), '\t');
        assert_eq!(get_delimeter_from_cli_argument(" ").unwrap(), ' ');
        assert_eq!(get_delimeter_from_cli_argument(",").unwrap(), ',');
        assert!(get_delimeter_from_cli_argument("::").is_err());
    }
}
