use crate::{AutoMultipartWrite, MultipartWrite};

use std::pin::Pin;
use std::task::{Context, Poll};

/// Writer for the [`forever`] method.
///
/// [`forever`]: super::MultipartWriteExt::forever
#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
#[pin_project::pin_project]
pub struct Forever<W> {
    #[pin]
    writer: W,
}

impl<W> Forever<W> {
    pub(super) fn new(writer: W) -> Self {
        Self { writer }
    }

    /// Acquires a mutable reference to the underlying writer.
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    /// Acquires a pinned mutable reference to the underlying writer.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut W> {
        self.project().writer
    }
}

impl<W: MultipartWrite<P>, P> MultipartWrite<P> for Forever<W> {
    type Ret = W::Ret;
    type Output = W::Output;
    type Error = W::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().writer.poll_ready(cx)
    }

    fn start_write(self: Pin<&mut Self>, part: P) -> Result<Self::Ret, Self::Error> {
        self.project().writer.start_write(part)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().writer.poll_flush(cx)
    }

    fn poll_freeze(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        self.project().writer.poll_freeze(cx)
    }
}

impl<W: MultipartWrite<P>, P> AutoMultipartWrite<P> for Forever<W> {
    fn should_freeze(self: Pin<&mut Self>) -> bool {
        false
    }
}
