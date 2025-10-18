use crate::MultipartWrite;
use crate::write::Feed;

use futures::{Future, ready};
use std::pin::Pin;
use std::task::{Context, Poll};

/// Future for [`send`](super::MultipartWriteExt::send).
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Send<'a, W: ?Sized + MultipartWrite<Part>, Part> {
    feed: Feed<'a, W, Part>,
    output: Option<W::Ret>,
}

impl<W: ?Sized + MultipartWrite<Part> + Unpin, Part> Unpin for Send<'_, W, Part> {}

impl<'a, W: ?Sized + MultipartWrite<Part> + Unpin, Part> Send<'a, W, Part> {
    pub(super) fn new(writer: &'a mut W, part: Part) -> Self {
        Self {
            feed: Feed::new(writer, part),
            output: None,
        }
    }
}

impl<W: MultipartWrite<Part> + Unpin, Part> Future for Send<'_, W, Part> {
    type Output = Result<W::Ret, W::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;

        if this.feed.is_part_pending() {
            let ret = ready!(Pin::new(&mut this.feed).poll(cx))?;
            this.output = Some(ret);
            debug_assert!(!this.feed.is_part_pending());
        }

        ready!(this.feed.writer_pin_mut().poll_flush(cx))?;
        let output = this.output.take().expect("polled Send after completion");

        Poll::Ready(Ok(output))
    }
}
