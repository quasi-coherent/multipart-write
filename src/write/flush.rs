use crate::MultipartWrite;

use futures::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Future for [`flush`](super::MultipartWriteExt::flush).
#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
pub struct Flush<'a, W: ?Sized, P> {
    writer: &'a mut W,
    _p: PhantomData<fn(P)>,
}

impl<W: ?Sized + Unpin, P> Unpin for Flush<'_, W, P> {}

impl<'a, W: ?Sized + MultipartWrite<P> + Unpin, P> Flush<'a, W, P> {
    pub(super) fn new(writer: &'a mut W) -> Self {
        Self {
            writer,
            _p: std::marker::PhantomData,
        }
    }
}

impl<W: MultipartWrite<P> + Unpin, P> Future for Flush<'_, W, P> {
    type Output = Result<(), W::Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.writer).poll_flush(cx)
    }
}
