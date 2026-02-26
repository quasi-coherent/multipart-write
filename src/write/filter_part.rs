use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::{FusedMultipartWrite, MultipartWrite};

pin_project_lite::pin_project! {
    /// `MultipartWrite` for [`filter_part`].
    ///
    /// [`filter_part`]: super::MultipartWriteExt::filter_part
    #[must_use = "futures do nothing unless polled"]
    pub struct FilterPart<Wr, F> {
        #[pin]
        writer: Wr,
        f: F,
    }
}

impl<Wr, F> FilterPart<Wr, F> {
    pub(super) fn new(writer: Wr, f: F) -> Self {
        Self { writer, f }
    }

    /// Consumes `FilterPart`, returning the underlying writer.
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

impl<Wr, F, Part> FusedMultipartWrite<Part> for FilterPart<Wr, F>
where
    Wr: FusedMultipartWrite<Part>,
    F: FnMut(&Part) -> bool,
{
    fn is_terminated(&self) -> bool {
        self.writer.is_terminated()
    }
}

impl<Wr, F, Part> MultipartWrite<Part> for FilterPart<Wr, F>
where
    Wr: MultipartWrite<Part>,
    F: FnMut(&Part) -> bool,
{
    type Error = Wr::Error;
    type Output = Wr::Output;
    type Recv = Option<Wr::Recv>;

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
        let this = self.project();
        if !(this.f)(&part) {
            return Ok(None);
        }
        let ret = this.writer.start_send(part)?;
        Ok(Some(ret))
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
        self.as_mut().project().writer.poll_complete(cx)
    }
}

impl<Wr: Debug, F> Debug for FilterPart<Wr, F> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilterPart").field("writer", &self.writer).finish()
    }
}
