use crate::MultipartWrite;

use std::pin::Pin;
use std::task::{Context, Poll};

/// `MultipartWrite` for the [`map_err`] method.
///
/// [`map_err`]: super::MultipartWriteExt::map_err
#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
#[pin_project::pin_project]
pub struct MapErr<W, F> {
    #[pin]
    writer: W,
    f: F,
}

impl<W, F> MapErr<W, F> {
    pub(super) fn new(writer: W, f: F) -> Self {
        Self { writer, f }
    }
}

impl<W, F, Part, E> MultipartWrite<Part> for MapErr<W, F>
where
    W: MultipartWrite<Part>,
    F: FnMut(W::Error) -> E,
{
    type Ret = W::Ret;
    type Output = W::Output;
    type Error = E;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.as_mut()
            .project()
            .writer
            .poll_ready(cx)
            .map_err(|e| (self.as_mut().project().f)(e))
    }

    fn start_write(mut self: Pin<&mut Self>, part: Part) -> Result<Self::Ret, Self::Error> {
        self.as_mut()
            .project()
            .writer
            .start_write(part)
            .map_err(|e| (self.as_mut().project().f)(e))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.as_mut()
            .project()
            .writer
            .poll_flush(cx)
            .map_err(|e| (self.as_mut().project().f)(e))
    }

    fn poll_freeze(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        self.as_mut()
            .project()
            .writer
            .poll_freeze(cx)
            .map_err(|e| (self.as_mut().project().f)(e))
    }
}
