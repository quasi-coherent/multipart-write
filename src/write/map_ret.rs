use crate::MultipartWrite;

use std::pin::Pin;
use std::task::{Context, Poll};

/// `MultipartWrite` for [`map_ret`](super::MultipartWriteExt::map_ret).
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
#[pin_project::pin_project]
pub struct MapRet<W, F> {
    #[pin]
    writer: W,
    f: F,
}

impl<W, F> MapRet<W, F> {
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

impl<U, W, F, P> MultipartWrite<P> for MapRet<W, F>
where
    W: MultipartWrite<P>,
    F: FnMut(W::Ret) -> U,
{
    type Ret = U;
    type Output = W::Output;
    type Error = W::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().writer.poll_ready(cx)
    }

    fn start_send(mut self: Pin<&mut Self>, part: P) -> Result<Self::Ret, Self::Error> {
        self.as_mut()
            .project()
            .writer
            .start_send(part)
            .map(self.as_mut().project().f)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().writer.poll_flush(cx)
    }

    fn poll_complete(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        self.project().writer.poll_complete(cx)
    }
}
