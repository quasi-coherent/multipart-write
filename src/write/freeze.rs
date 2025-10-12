use crate::MultipartWrite;

use futures::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Future for the [`freeze`](super::MultipartWriteExt::freeze) method.
#[must_use = "futures do nothing unless polled"]
#[pin_project::pin_project]
pub struct Freeze<'a, W: ?Sized, P> {
    #[pin]
    writer: &'a mut W,
    _p: std::marker::PhantomData<P>,
    #[pin]
    _u: std::marker::PhantomPinned,
}

impl<'a, W: MultipartWrite<P> + ?Sized + Unpin, P> Freeze<'a, W, P> {
    pub(super) fn new(writer: &'a mut W) -> Self {
        Self {
            writer,
            _p: std::marker::PhantomData,
            _u: std::marker::PhantomPinned,
        }
    }
}

impl<W: MultipartWrite<P> + Unpin, P> Future for Freeze<'_, W, P> {
    type Output = Result<W::Output, W::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().writer.as_mut().poll_freeze(cx)
    }
}
