use crate::MultipartWrite;

use std::pin::Pin;
use std::task::{Context, Poll};

/// `MultipartWrite` for [`map_err`](super::MultipartWriteExt::map_err).
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
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

    /// Acquires a reference to the underlying writer.
    pub fn get_ref(&self) -> &W {
        &self.writer
    }

    /// Acquires a mutable reference to the underlying writer.
    ///
    /// It is inadvisable to directly write to the underlying writer.
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    /// Acquires a pinned mutable reference to the underlying writer.
    ///
    /// It is inadvisable to directly write to the underlying writer.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut W> {
        self.project().writer
    }
}

impl<W, F, P, E> MultipartWrite<P> for MapErr<W, F>
where
    W: MultipartWrite<P>,
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
            .map_err(self.as_mut().project().f)
    }

    fn start_send(mut self: Pin<&mut Self>, part: P) -> Result<Self::Ret, Self::Error> {
        self.as_mut()
            .project()
            .writer
            .start_send(part)
            .map_err(self.as_mut().project().f)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.as_mut()
            .project()
            .writer
            .poll_flush(cx)
            .map_err(self.as_mut().project().f)
    }

    fn poll_complete(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        self.as_mut()
            .project()
            .writer
            .poll_complete(cx)
            .map_err(self.as_mut().project().f)
    }
}
