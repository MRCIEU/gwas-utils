use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use std::error::Error;
use std::fs::File;
use std::io::{Read, Seek, Write};

pub enum Reader {
    File(std::fs::File),
    GzFile(GzDecoder<std::fs::File>),
}

impl Read for Reader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Reader::File(f) => f.read(buf),
            Reader::GzFile(gz) => gz.read(buf),
        }
    }
}

impl Reader {
    pub fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        match self {
            Reader::File(f) => f.seek(pos),
            Reader::GzFile(gz) => gz.get_mut().seek(pos),
        }
    }
}

pub fn open_reader(filename: &str) -> Result<Reader, Box<dyn Error>> {
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
}

impl Write for Writer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Writer::File(f) => f.write(buf),
            Writer::GzFile(gz) => gz.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Writer::File(f) => f.flush(),
            Writer::GzFile(gz) => gz.flush(),
        }
    }
}

pub fn open_writer(filename: &str) -> Result<Writer, Box<dyn Error>> {
    let f = File::create(filename)?;
    if filename.ends_with(".gz") {
        let gz = GzEncoder::new(f, Compression::default());
        Ok(Writer::GzFile(gz))
    } else {
        Ok(Writer::File(f))
    }
}

pub fn get_delimeter(sep: &str) -> Result<char, Box<dyn Error>> {
    let c = match sep {
        "\\t" => '\t',
        s if s.chars().count() == 1 => s.chars().next().unwrap(),
        _ => return Err("Delimiter must be a single ASCII character".into()),
    };
    if !c.is_ascii() {
        return Err("Delimiter must be a single ASCII character".into());
    }
    Ok(c)
}
