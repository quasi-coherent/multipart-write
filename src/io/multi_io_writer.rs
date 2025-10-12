use crate::MultipartWrite;

use std::io::Write;
use std::pin::Pin;
use std::task::{Context, Poll};

/// `MultiIoWriter` implements [`MultipartWrite`] for a synchronous
/// [`Write`](std::io::Write).
///
/// [`MultipartWrite`]: crate::MultipartWrite
#[pin_project::pin_project]
#[derive(Debug, Default)]
pub struct MultiIoWriter<W: Write> {
    inner: W,
}

impl<W: Write> MultiIoWriter<W> {
    pub(super) fn new(inner: W) -> Self {
        Self { inner }
    }
}

impl<W: Write + Default> MultipartWrite<&[u8]> for MultiIoWriter<W> {
    type Ret = usize;
    type Output = W;
    type Error = std::io::Error;

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_write(self: Pin<&mut Self>, part: &[u8]) -> Result<Self::Ret, Self::Error> {
        self.get_mut().inner.write(part)
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(self.get_mut().inner.flush())
    }

    fn poll_freeze(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        Poll::Ready(Ok(std::mem::take(&mut self.inner)))
    }
}
