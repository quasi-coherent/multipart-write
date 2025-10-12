use crate::MultipartWrite;

use std::pin::Pin;
use std::task::{Context, Poll};

/// `MultipartWrite` for the [`map`] method.
///
/// [`map`]: super::MultipartWriteExt::map
#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
#[pin_project::pin_project]
pub struct Map<W, F> {
    #[pin]
    writer: W,
    f: F,
}

impl<W, F> Map<W, F> {
    pub(super) fn new(writer: W, f: F) -> Self {
        Self { writer, f }
    }
}

impl<U, W, F, Part> MultipartWrite<Part> for Map<W, F>
where
    W: MultipartWrite<Part>,
    F: FnMut(W::Output) -> U,
{
    type Ret = W::Ret;
    type Output = U;
    type Error = W::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().writer.poll_ready(cx)
    }

    fn start_write(self: Pin<&mut Self>, part: Part) -> Result<Self::Ret, Self::Error> {
        self.project().writer.start_write(part)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().writer.poll_flush(cx)
    }

    fn poll_freeze(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        self.as_mut()
            .project()
            .writer
            .poll_freeze(cx)
            .map_ok(|v| (self.as_mut().project().f)(v))
    }
}
