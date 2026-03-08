use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::{FusedMultipartWrite, MultipartWrite};

pin_project_lite::pin_project! {
    /// `MultipartWrite` for [`map_ok`](super::MultipartWriteExt::map_ok).
    #[must_use = "futures do nothing unless polled"]
    pub struct MapOk<Wr, Part, T, F> {
        #[pin]
        writer: Wr,
        f: F,
        _p: PhantomData<fn(Part) -> T>,
    }
}

impl<Wr, Part, T, F> MapOk<Wr, Part, T, F> {
    pub(super) fn new(writer: Wr, f: F) -> Self {
        Self { writer, f, _p: PhantomData }
    }

    /// Consumes `MapOk`, returning the underlying writer.
    pub fn into_inner(self) -> Wr {
        self.writer
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

impl<Wr, Part, T, F> FusedMultipartWrite<Part> for MapOk<Wr, Part, T, F>
where
    Wr: FusedMultipartWrite<Part>,
    F: FnMut(Wr::Output) -> T,
{
    fn is_terminated(&self) -> bool {
        self.writer.is_terminated()
    }
}

impl<Wr, Part, T, F> MultipartWrite<Part> for MapOk<Wr, Part, T, F>
where
    Wr: MultipartWrite<Part>,
    F: FnMut(Wr::Output) -> T,
{
    type Error = Wr::Error;
    type Output = T;
    type Recv = Wr::Recv;

    fn poll_ready(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.project().writer.poll_ready(cx)
    }

    fn start_send(
        self: Pin<&mut Self>,
        part: Part,
    ) -> Result<Self::Recv, Self::Error> {
        self.project().writer.start_send(part)
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
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

impl<Wr: Debug, Part, T, F> Debug for MapOk<Wr, Part, T, F> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapOk").field("writer", &self.writer).finish()
    }
}
