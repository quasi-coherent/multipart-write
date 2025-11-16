use crate::{FusedMultipartWrite, MultipartWrite};

use futures_core::ready;
use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project_lite::pin_project! {
    /// `MultipartWrite` for [`fuse`](super::MultipartWriteExt::fuse).
    #[must_use = "futures do nothing unless polled"]
    pub struct Fuse<Wr, F> {
        #[pin]
        writer: Wr,
        f: F,
        is_terminated: bool,
    }
}

impl<Wr, F> Fuse<Wr, F> {
    pub(super) fn new(writer: Wr, f: F) -> Self {
        Self {
            writer,
            f,
            is_terminated: false,
        }
    }

    /// Consumes `Fuse`, returning the underlying writer.
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

impl<Item, Wr, F> FusedMultipartWrite<Item> for Fuse<Wr, F>
where
    Wr: MultipartWrite<Item>,
    F: FnMut(&Wr::Output) -> bool,
{
    fn is_terminated(&self) -> bool {
        self.is_terminated
    }
}

impl<Item, Wr, F> MultipartWrite<Item> for Fuse<Wr, F>
where
    Wr: MultipartWrite<Item>,
    F: FnMut(&Wr::Output) -> bool,
{
    type Ret = Option<Wr::Ret>;
    type Output = Option<Wr::Output>;
    type Error = Wr::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.is_terminated {
            return Poll::Ready(Ok(()));
        }
        self.project().writer.as_mut().poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, part: Item) -> Result<Self::Ret, Self::Error> {
        if self.is_terminated {
            return Ok(None);
        }
        self.project().writer.as_mut().start_send(part).map(Some)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.is_terminated {
            return Poll::Ready(Ok(()));
        }
        self.project().writer.as_mut().poll_flush(cx)
    }

    fn poll_complete(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        let mut this = self.project();
        if *this.is_terminated {
            return Poll::Ready(Ok(None));
        }
        let res = ready!(this.writer.as_mut().poll_complete(cx))?;
        if (this.f)(&res) {
            *this.is_terminated = true;
        }
        Poll::Ready(Ok(Some(res)))
    }
}

impl<Wr, F> Debug for Fuse<Wr, F>
where
    Wr: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Fuse")
            .field("writer", &self.writer)
            .field("f", &"FnMut(&Wr::Output) -> bool")
            .field("is_terminated", &self.is_terminated)
            .finish()
    }
}
