use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::{FusedMultipartWrite, MultipartWrite};

pin_project_lite::pin_project! {
    /// `MultipartWrite` for [`map_sent`](super::MultipartWriteExt::map_sent).
    #[must_use = "futures do nothing unless polled"]
    pub struct MapSent<Wr, Part, R, F> {
        #[pin]
        writer: Wr,
        f: F,
        _p: PhantomData<fn(R) -> Part>,
    }
}

impl<Wr, Part, R, F> MapSent<Wr, Part, R, F> {
    pub(super) fn new(writer: Wr, f: F) -> Self {
        Self { writer, f, _p: PhantomData }
    }

    /// Consumes `MapSent`, returning the underlying writer.
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

impl<Wr, Part, R, F> FusedMultipartWrite<Part> for MapSent<Wr, Part, R, F>
where
    Wr: FusedMultipartWrite<Part>,
    F: FnMut(Wr::Recv) -> R,
{
    fn is_terminated(&self) -> bool {
        self.writer.is_terminated()
    }
}

impl<Wr, Part, R, F> MultipartWrite<Part> for MapSent<Wr, Part, R, F>
where
    Wr: MultipartWrite<Part>,
    F: FnMut(Wr::Recv) -> R,
{
    type Error = Wr::Error;
    type Output = Wr::Output;
    type Recv = R;

    fn poll_ready(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.project().writer.poll_ready(cx)
    }

    fn start_send(
        mut self: Pin<&mut Self>,
        part: Part,
    ) -> Result<Self::Recv, Self::Error> {
        self.as_mut()
            .project()
            .writer
            .start_send(part)
            .map(self.as_mut().project().f)
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.project().writer.poll_flush(cx)
    }

    fn poll_complete(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        self.project().writer.poll_complete(cx)
    }
}

impl<Wr: Debug, Part, R, F> Debug for MapSent<Wr, Part, R, F> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapSent").field("writer", &self.writer).finish()
    }
}
