use crate::MultipartWrite;

use futures::future::Future;
use std::pin::Pin;
use std::task::{self, Context, Poll};

/// Future for the [`write_part`](super::MultipartWriteExt::write_part) method.
#[must_use = "futures do nothing unless polled"]
#[pin_project::pin_project]
pub struct WritePart<'a, W: ?Sized, P> {
    #[pin]
    writer: &'a mut W,
    part: Option<P>,
    #[pin]
    _u: std::marker::PhantomPinned,
}

impl<'a, W: MultipartWrite<P> + ?Sized + Unpin, P> WritePart<'a, W, P> {
    pub(super) fn new(writer: &'a mut W, part: P) -> Self {
        Self {
            writer,
            part: Some(part),
            _u: std::marker::PhantomPinned,
        }
    }
}

impl<W: MultipartWrite<P> + Unpin, P> Future for WritePart<'_, W, P> {
    type Output = Result<W::Ret, W::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        task::ready!(this.writer.as_mut().poll_ready(cx))?;
        let part = this.part.take().expect("polled Write after completion");

        Poll::Ready(this.writer.start_write(part))
    }
}
