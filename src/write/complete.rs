use crate::{FusedMultipartWrite, MultipartWrite};

use futures_core::future::{FusedFuture, Future};
use futures_core::ready;
use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

/// Future for [`complete`](super::MultipartWriteExt::complete).
#[must_use = "futures do nothing unless polled"]
pub struct Complete<'a, Wr: ?Sized, Part> {
    writer: &'a mut Wr,
    is_terminated: bool,
    _p: std::marker::PhantomData<Part>,
}

impl<Wr: ?Sized + Unpin, Part> Unpin for Complete<'_, Wr, Part> {}

impl<'a, Wr: MultipartWrite<Part> + ?Sized + Unpin, Part> Complete<'a, Wr, Part> {
    pub(super) fn new(writer: &'a mut Wr) -> Self {
        Self {
            writer,
            is_terminated: false,
            _p: std::marker::PhantomData,
        }
    }
}

impl<Wr: ?Sized + FusedMultipartWrite<Part> + Unpin, Part> FusedFuture for Complete<'_, Wr, Part> {
    fn is_terminated(&self) -> bool {
        self.writer.is_terminated()
    }
}

impl<Wr: ?Sized + MultipartWrite<Part> + Unpin, Part> Future for Complete<'_, Wr, Part> {
    type Output = Result<Wr::Output, Wr::Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let out = ready!(Pin::new(&mut self.writer).poll_complete(cx));
        self.is_terminated = true;
        Poll::Ready(out)
    }
}

impl<Wr: Debug, Part> Debug for Complete<'_, Wr, Part> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Complete")
            .field("writer", &self.writer)
            .field("is_terminated", &self.is_terminated)
            .finish()
    }
}
