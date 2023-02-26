use std::{
    io,
    io::{Error, Write},
    pin::Pin,
    task::{Context, Poll},
};

use sha2::Digest;
use tokio::io::{AsyncRead, AsyncWrite};

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
    pub fn into_inner(self) -> (T, Sha2Hash) {
        (self.inner, self.hasher.finalize().into())
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

impl<T: AsyncRead + Unpin> AsyncRead for HashIo<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<Result<(), Error>> {
        let start = buf.filled().len();
        let inner_poll = Pin::new(&mut self.inner).poll_read(cx, buf);
        if let Poll::Ready(Ok(_)) = inner_poll {
            self.hasher.update(&buf.filled()[start..]);
        }
        inner_poll
    }
}

impl<T: AsyncWrite + Unpin> AsyncWrite for HashIo<T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, Error>> {
        let inner_poll = Pin::new(&mut self.inner).poll_write(cx, buf);
        if let Poll::Ready(Ok(n)) = inner_poll {
            self.hasher.update(&buf[..n]);
        }
        inner_poll
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod test_async {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;
    use crate::hash::test::{MAGIC_PARTIAL, TEST_DATA, TEST_HASH_MAGIC_STR, TEST_HASH_STR};

    struct ASink;

    impl AsyncWrite for ASink {
        fn poll_write(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<Result<usize, Error>> {
            Poll::Ready(Ok(buf.len()))
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
            Poll::Ready(Ok(()))
        }
    }

    async fn read_slow<T: AsRef<[u8]>>(data: T) -> Sha2Hash {
        assert!(data.as_ref().len() > MAGIC_PARTIAL);
        let mut r = HashIo::new(std::io::Cursor::new(data.as_ref()));
        let mut b = [0u8; MAGIC_PARTIAL]; // really small buffer
        loop {
            let c = r.read(&mut b).await.unwrap();
            if c == 0 {
                return r.into_inner().1;
            }
        }
    }
    async fn write_slow<T: AsRef<[u8]>>(data: T, bail_after_four: bool) -> Sha2Hash {
        assert!(data.as_ref().len() > MAGIC_PARTIAL);
        let mut r = HashIo::new(ASink);
        let mut total = 0;
        for b in data.as_ref().chunks(MAGIC_PARTIAL) {
            let c = r.write(b).await.unwrap();
            total += c;
            assert_eq!(c, b.len());
            if bail_after_four {
                return r.into_inner().1;
            }
        }
        assert_eq!(total, data.as_ref().len());
        r.into_inner().1
    }

    async fn read_four_bytes<T: AsRef<[u8]>>(data: T) -> Sha2Hash {
        assert!(data.as_ref().len() > MAGIC_PARTIAL);
        let mut r = HashIo::new(std::io::Cursor::new(data.as_ref()));
        let mut b = [0u8; MAGIC_PARTIAL]; // really small buffer
        r.read_exact(&mut b).await.unwrap();
        r.into_inner().1
    }

    #[tokio::test]
    async fn async_read_and_hash() {
        let hash = read_slow(TEST_DATA).await;
        assert_eq!(format!("{}", hash), TEST_HASH_STR,);
    }
    #[tokio::test]
    async fn async_read_partial_and_hash() {
        let hash = read_four_bytes(TEST_DATA).await;
        assert_eq!(format!("{:?}", hash), TEST_HASH_MAGIC_STR,);
    }

    #[tokio::test]
    async fn async_write_and_hash() {
        let hash = write_slow(TEST_DATA, false).await;
        assert_eq!(format!("{}", hash), TEST_HASH_STR,);
    }

    #[tokio::test]
    async fn async_write_partial_and_hash() {
        let hash = write_slow(TEST_DATA, true).await;
        assert_eq!(format!("{:?}", hash), TEST_HASH_MAGIC_STR,);
    }

    #[tokio::test]
    async fn async_write_large_data_and_hash() {
        // I was worried that the implementation of asyncread was incorrect,
        // and we wouldn't read from the buffer correctly. This test is to
        // make sure the buffer gets filled/reset, and validate that the
        // hash is correct.

        let target = 1024 * 1024; // MB
        let mut data = TEST_DATA.repeat(target / TEST_DATA.len());
        let extra = TEST_DATA.split_at(target - data.len());
        data.push_str(extra.0);
        assert_eq!(data.len(), target);

        let expected = Sha2Hash::digest(&data);
        let actual = write_slow(data, false).await;
        assert_eq!(format!("{:?}", actual), format!("{:?}", expected),);
    }
}

#[cfg(test)]
mod test {
    use std::io::{Read, Write};

    use super::*;
    use crate::hash::test::{TEST_DATA, TEST_HASH_STR};

    #[test]
    fn write_data_through_hasher() {
        let mut c = HashIo::new(std::io::Cursor::new(Vec::new()));
        c.write_all(TEST_DATA.as_bytes()).unwrap();
        let (res, hash) = c.into_inner();
        let res = res.into_inner();
        assert_eq!(res, TEST_DATA.as_bytes());
        assert_eq!(hash.to_string(), TEST_HASH_STR);
    }

    #[test]
    fn read_data_through_reader() {
        let mut c = HashIo::new(std::io::Cursor::new(TEST_DATA));
        let mut sink = String::new();
        c.read_to_string(&mut sink).unwrap();
        let (_, hash) = c.into_inner();
        assert_eq!(sink, TEST_DATA);
        assert_eq!(hash.to_string(), TEST_HASH_STR);
    }

    #[test]
    fn copy_data_through_read_write() {
        let mut source = HashIo::new(std::io::Cursor::new(TEST_DATA));
        let mut sink = HashIo::new(std::io::Cursor::new(Vec::new()));
        io::copy(&mut source, &mut sink).unwrap();

        let (_, r_hash) = source.into_inner();
        let (res, w_hash) = sink.into_inner();
        let res = res.into_inner();

        assert_eq!(res, TEST_DATA.as_bytes());
        assert_eq!(r_hash.to_string(), TEST_HASH_STR);
        assert_eq!(w_hash.to_string(), TEST_HASH_STR);
    }
}
