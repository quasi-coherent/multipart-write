use crate::MultipartWrite;

use std::pin::Pin;
use std::task::{self, Context, Poll};
use tokio::io::AsyncWrite;

// https://github.com/rust-lang/rust/blob/ff6dc928c5e33ce8e65c6911a790b9efcb5ef53a/library/std/src/sys/io/mod.rs#L54
const DEFAULT_BUF_SIZE: usize = 8 * 1024;

/// Converts an [`AsyncWrite`] into a [`MultipartWrite`].
///
/// [`AsyncWrite`]: tokio::io::AsyncWrite
/// [`MultipartWrite`]: crate::MultipartWrite
pub fn async_write<W: AsyncWrite + Unpin + Default>(write: W) -> MultiAsyncWriter<W> {
    MultiAsyncWriter::new(write)
}

/// `MultiAsyncWriter` implements [`MultipartWrite`] for an asynchronous
/// [`tokio::io::AsyncWrite`](tokio::io::AsyncWrite).
///
/// [`MultipartWrite`]: crate::MultipartWrite
#[pin_project::pin_project]
#[derive(Clone, Default)]
pub struct MultiAsyncWriter<W: AsyncWrite> {
    #[pin]
    inner: W,
    buf: Vec<u8>,
    written: usize,
}

impl<W: AsyncWrite + Unpin> MultiAsyncWriter<W> {
    pub(super) fn new(inner: W) -> Self {
        Self {
            inner,
            buf: Vec::with_capacity(DEFAULT_BUF_SIZE),
            written: 0,
        }
    }

    fn flush_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let mut this = self.project();

        let len = this.buf.len();
        let mut ret = Ok(());
        while *this.written < len {
            match task::ready!(
                this.inner
                    .as_mut()
                    .poll_write(cx, &this.buf[*this.written..])
            ) {
                Ok(0) => {
                    ret = Err(std::io::Error::new(
                        std::io::ErrorKind::WriteZero,
                        "failed to write buffered data",
                    ));
                    break;
                }
                Ok(n) => *this.written += n,
                Err(e) => {
                    ret = Err(e);
                    break;
                }
            }
        }
        if *this.written > 0 {
            this.buf.drain(..*this.written);
        }
        *this.written = 0;

        Poll::Ready(ret)
    }
}

impl<W: AsyncWrite + Default + Unpin> MultipartWrite<&[u8]> for MultiAsyncWriter<W> {
    type Ret = usize;
    type Output = W;
    type Error = std::io::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.flush_buf(cx)
    }

    fn start_write(self: Pin<&mut Self>, part: &[u8]) -> Result<Self::Ret, Self::Error> {
        let len = part.len();
        self.project().buf.extend_from_slice(part);
        Ok(len)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_flush(cx)
    }

    fn poll_freeze(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        Poll::Ready(Ok(std::mem::take(&mut self.inner)))
    }
}
