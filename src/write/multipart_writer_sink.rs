use crate::MultipartWrite;

use futures::sink::Sink;
use std::pin::Pin;
use std::task::{self, Context, Poll};

/// `MultipartWriterSink` wraps a [`Sink`] or [`MultipartWrite`] and provides an
/// implementation of the other trait.
///
/// [`Sink`]: futures::sink::Sink
#[derive(Debug)]
#[pin_project::pin_project]
pub struct MultipartWriterSink<W> {
    #[pin]
    inner: W,
}

impl<W> MultipartWriterSink<W> {
    pub fn new(inner: W) -> Self {
        Self { inner }
    }
}

impl<W, P> Sink<P> for MultipartWriterSink<W>
where
    W: MultipartWrite<P>,
{
    type Error = W::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: P) -> Result<(), Self::Error> {
        let _ = self.project().inner.start_write(item)?;
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let _ = task::ready!(self.project().inner.poll_freeze(cx))?;
        Poll::Ready(Ok(()))
    }
}

impl<W, P> MultipartWrite<P> for MultipartWriterSink<W>
where
    W: Sink<P>,
{
    type Ret = ();
    type Output = ();
    type Error = W::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_ready(cx)
    }

    fn start_write(self: Pin<&mut Self>, part: P) -> Result<Self::Ret, Self::Error> {
        self.project().inner.start_send(part)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_flush(cx)
    }

    fn poll_freeze(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        self.project().inner.poll_close(cx)
    }
}
