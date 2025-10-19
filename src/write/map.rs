use crate::{FusedMultipartWrite, MultipartWrite};

use std::pin::Pin;
use std::task::{Context, Poll};

/// `MultipartWrite` for [`map`](super::MultipartWriteExt::map).
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
#[pin_project::pin_project]
pub struct Map<Wr, F> {
    #[pin]
    writer: Wr,
    f: F,
}

impl<Wr, F> Map<Wr, F> {
    pub(super) fn new(writer: Wr, f: F) -> Self {
        Self { writer, f }
    }

    /// Acquires a reference to the underlying writer.
    pub fn get_ref(&self) -> &Wr {
        &self.writer
    }

    /// Acquires a mutable reference to the underlying writer.
    ///
    /// It is inadvisable to directly write to the underlying writer.
    pub fn get_mut(&mut self) -> &mut Wr {
        &mut self.writer
    }

    /// Acquires a pinned mutable reference to the underlying writer.
    ///
    /// It is inadvisable to directly write to the underlying writer.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut Wr> {
        self.project().writer
    }
}

impl<U, Wr, F, Part> FusedMultipartWrite<Part> for Map<Wr, F>
where
    Wr: FusedMultipartWrite<Part>,
    F: FnMut(Wr::Output) -> U,
{
    fn is_terminated(&self) -> bool {
        self.writer.is_terminated()
    }
}

impl<U, Wr, F, Part> MultipartWrite<Part> for Map<Wr, F>
where
    Wr: MultipartWrite<Part>,
    F: FnMut(Wr::Output) -> U,
{
    type Ret = Wr::Ret;
    type Output = U;
    type Error = Wr::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().writer.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, part: Part) -> Result<Self::Ret, Self::Error> {
        self.project().writer.start_send(part)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().writer.poll_flush(cx)
    }

    fn poll_complete(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        self.as_mut()
            .project()
            .writer
            .poll_complete(cx)
            .map_ok(self.as_mut().project().f)
    }
}
