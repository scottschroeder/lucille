use std::{io, io::Write};

use sha2::Digest;

use super::{MySha, Sha2Hash};

pub struct HashIo<T> {
    hasher: MySha,
    inner: T,
}

impl<T> HashIo<T> {
    pub fn new(inner: T) -> HashIo<T> {
        HashIo {
            hasher: MySha::default(),
            inner,
        }
    }
    pub fn into_inner(self) -> (Sha2Hash, T) {
        (Sha2Hash::from(self.hasher.finalize()), self.inner)
    }
}

impl<W: io::Write> io::Write for HashIo<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let x = self.inner.write(buf)?;
        self.hasher.write(&buf[..x])
    }

    fn flush(&mut self) -> io::Result<()> {
        self.hasher.flush()?;
        self.inner.flush()
    }
}

impl<R: io::Read> io::Read for HashIo<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let x = self.inner.read(buf)?;
        let _ = self.hasher.write(&buf[..x])?;
        Ok(x)
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    use super::*;
    use crate::hash::test::{TEST_DATA, TEST_HASH_STR};

    #[test]
    fn write_data_through_hasher() {
        let mut c = HashIo::new(std::io::Cursor::new(Vec::new()));
        c.write_all(TEST_DATA.as_bytes()).unwrap();
        let (hash, res) = c.into_inner();
        let res = res.into_inner();
        assert_eq!(res, TEST_DATA.as_bytes());
        assert_eq!(hash.to_string(), TEST_HASH_STR);
    }

    #[test]
    fn read_data_through_reader() {
        let mut c = HashIo::new(std::io::Cursor::new(TEST_DATA));
        let mut sink = String::new();
        c.read_to_string(&mut sink).unwrap();
        let (hash, _) = c.into_inner();
        // let res = res.into_inner();
        assert_eq!(sink, TEST_DATA);
        assert_eq!(hash.to_string(), TEST_HASH_STR);
    }

    #[test]
    fn copy_data_through_read_write() {
        let mut source = HashIo::new(std::io::Cursor::new(TEST_DATA));
        let mut sink = HashIo::new(std::io::Cursor::new(Vec::new()));
        io::copy(&mut source, &mut sink).unwrap();

        let (r_hash, _) = source.into_inner();
        let (w_hash, res) = sink.into_inner();
        let res = res.into_inner();

        assert_eq!(res, TEST_DATA.as_bytes());
        assert_eq!(r_hash.to_string(), TEST_HASH_STR);
        assert_eq!(w_hash.to_string(), TEST_HASH_STR);
    }
}
