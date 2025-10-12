use crate::MultipartWrite;

use futures::sink::Sink;
use std::pin::Pin;
use std::task::{self, Context, Poll};

/// `MultipartWriterSink` wraps a [`Sink`] or [`MultipartWrite`] and provides an
/// implementation of the other trait.
///
/// [`Sink`]: futures::sink::Sink
#[pin_project::pin_project]
pub struct MultipartWriterSink<W> {
    #[pin]
    writer: W,
}

impl<W> MultipartWriterSink<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }
}

impl<W, P> Sink<P> for MultipartWriterSink<W>
where
    W: MultipartWrite<P>,
{
    type Error = W::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().writer.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: P) -> Result<(), Self::Error> {
        let _ = self.project().writer.start_write(item)?;
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().writer.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let _ = task::ready!(self.project().writer.poll_freeze(cx))?;
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
        self.project().writer.poll_ready(cx)
    }

    fn start_write(self: Pin<&mut Self>, part: P) -> Result<Self::Ret, Self::Error> {
        self.project().writer.start_send(part)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().writer.poll_flush(cx)
    }

    fn poll_freeze(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        self.project().writer.poll_close(cx)
    }
}
