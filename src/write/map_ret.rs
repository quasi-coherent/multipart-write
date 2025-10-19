use crate::{FusedMultipartWrite, MultipartWrite};

use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project_lite::pin_project! {
    /// `MultipartWrite` for [`map_ret`](super::MultipartWriteExt::map_ret).
    #[must_use = "futures do nothing unless polled"]
    pub struct MapRet<Wr, F> {
        #[pin]
        writer: Wr,
        f: F,
    }
}

impl<Wr, F> MapRet<Wr, F> {
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

impl<U, Wr, F, Part> FusedMultipartWrite<Part> for MapRet<Wr, F>
where
    Wr: FusedMultipartWrite<Part>,
    F: FnMut(Wr::Ret) -> U,
{
    fn is_terminated(&self) -> bool {
        self.writer.is_terminated()
    }
}

impl<U, Wr, F, Part> MultipartWrite<Part> for MapRet<Wr, F>
where
    Wr: MultipartWrite<Part>,
    F: FnMut(Wr::Ret) -> U,
{
    type Ret = U;
    type Output = Wr::Output;
    type Error = Wr::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().writer.poll_ready(cx)
    }

    fn start_send(mut self: Pin<&mut Self>, part: Part) -> Result<Self::Ret, Self::Error> {
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

impl<Wr: Debug, F> Debug for MapRet<Wr, F> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapRet")
            .field("writer", &self.writer)
            .field("f", &"F")
            .finish()
    }
}
