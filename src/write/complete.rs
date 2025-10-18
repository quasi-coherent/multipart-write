use crate::MultipartWrite;
use crate::write::MultipartWriteExt;

use futures::future::{FusedFuture, Future};
use futures::ready;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Future for [`complete`](super::MultipartWriteExt::complete).
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Complete<'a, W: ?Sized, P> {
    writer: &'a mut W,
    is_terminated: bool,
    _p: std::marker::PhantomData<P>,
}

impl<W: ?Sized + Unpin, P> Unpin for Complete<'_, W, P> {}

impl<'a, W: MultipartWrite<P> + ?Sized + Unpin, P> Complete<'a, W, P> {
    pub(super) fn new(writer: &'a mut W) -> Self {
        Self {
            writer,
            is_terminated: false,
            _p: std::marker::PhantomData,
        }
    }
}

impl<W: ?Sized + MultipartWrite<P> + Unpin, P> FusedFuture for Complete<'_, W, P> {
    fn is_terminated(&self) -> bool {
        self.is_terminated
    }
}

impl<W: ?Sized + MultipartWrite<P> + Unpin, P> Future for Complete<'_, W, P> {
    type Output = Result<W::Output, W::Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        ready!(self.writer.poll_flush_unpin(cx))?;
        let output = ready!(self.writer.poll_complete_unpin(cx));
        self.is_terminated = true;
        Poll::Ready(output)
    }
}
