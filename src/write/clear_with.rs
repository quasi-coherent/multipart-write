use crate::{FusedMultipartWrite, MultipartWrite};

use futures_core::ready;
use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project_lite::pin_project! {
    /// `MultipartWrite` for [`clear_with`].
    ///
    /// [`clear_with`]: super::MultipartWriteExt::clear_with
    #[must_use = "futures do nothing unless polled"]
    pub struct ClearWith<Wr, T, F> {
        #[pin]
        writer: Wr,
        val: T,
        f: F,
    }
}

impl<Wr: Unpin, T, F> ClearWith<Wr, T, F> {
    pub(super) fn new(writer: Wr, val: T, f: F) -> Self {
        Self { writer, val, f }
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

impl<Wr, Part, T, F> FusedMultipartWrite<Part> for ClearWith<Wr, T, F>
where
    Wr: FusedMultipartWrite<Part> + Unpin,
    F: FnMut(&mut Wr, &T),
{
    fn is_terminated(&self) -> bool {
        self.writer.is_terminated()
    }
}

impl<Wr, Part, T, F> MultipartWrite<Part> for ClearWith<Wr, T, F>
where
    Wr: MultipartWrite<Part> + Unpin,
    F: FnMut(&mut Wr, &T),
{
    type Ret = Wr::Ret;
    type Output = Wr::Output;
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
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        let mut this = self.project();
        let out = ready!(this.writer.as_mut().poll_complete(cx));
        (this.f)(&mut this.writer, &*this.val);
        Poll::Ready(out)
    }
}

impl<Wr, T, F> Debug for ClearWith<Wr, T, F>
where
    Wr: Debug,
    T: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClearWith")
            .field("writer", &self.writer)
            .field("val", &self.val)
            .field("f", &"impl FnMut(&mut Self, &T)")
            .finish()
    }
}
