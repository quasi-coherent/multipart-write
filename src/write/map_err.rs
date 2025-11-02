use crate::{FusedMultipartWrite, MultipartWrite};

use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project_lite::pin_project! {
    /// `MultipartWrite` for [`map_err`](super::MultipartWriteExt::map_err).
    #[must_use = "futures do nothing unless polled"]
    pub struct MapErr<Wr, F> {
        #[pin]
        writer: Wr,
        f: F,
    }
}

impl<Wr, F> MapErr<Wr, F> {
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

impl<Wr, F, Part, E> FusedMultipartWrite<Part> for MapErr<Wr, F>
where
    Wr: FusedMultipartWrite<Part>,
    F: FnMut(Wr::Error) -> E,
{
    fn is_terminated(&self) -> bool {
        self.writer.is_terminated()
    }
}

impl<Wr, F, Part, E> MultipartWrite<Part> for MapErr<Wr, F>
where
    Wr: MultipartWrite<Part>,
    F: FnMut(Wr::Error) -> E,
{
    type Ret = Wr::Ret;
    type Output = Wr::Output;
    type Error = E;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.as_mut()
            .project()
            .writer
            .poll_ready(cx)
            .map_err(self.as_mut().project().f)
    }

    fn start_send(mut self: Pin<&mut Self>, part: Part) -> Result<Self::Ret, Self::Error> {
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

impl<Wr: Debug, F> Debug for MapErr<Wr, F> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapErr")
            .field("writer", &self.writer)
            .field("f", &"impl FnMut(Wr::Error) -> E")
            .finish()
    }
}
