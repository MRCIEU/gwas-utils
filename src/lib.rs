use std::error::Error;
use std::io::{Read, Write};

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;

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

pub fn open_reader(filename: &str) -> Result<Reader, Box<dyn Error>> {
    let f = std::fs::File::open(filename)?;
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
    let f = std::fs::File::create(filename)?;
    if filename.ends_with(".gz") {
        let gz = GzEncoder::new(f, flate2::Compression::default());
        Ok(Writer::GzFile(gz))
    } else {
        Ok(Writer::File(f))
    }
}
