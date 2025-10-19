use crate::write::Feed;
use crate::{FusedMultipartWrite, MultipartWrite};

use futures::future::FusedFuture;
use futures::ready;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Future for [`send`](super::MultipartWriteExt::send).
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Send<'a, Wr: ?Sized + MultipartWrite<Part>, Part> {
    feed: Feed<'a, Wr, Part>,
    output: Option<Wr::Ret>,
}

impl<Wr: ?Sized + MultipartWrite<Part> + Unpin, Part> Unpin for Send<'_, Wr, Part> {}

impl<'a, Wr: ?Sized + MultipartWrite<Part> + Unpin, Part> Send<'a, Wr, Part> {
    pub(super) fn new(writer: &'a mut Wr, part: Part) -> Self {
        Self {
            feed: Feed::new(writer, part),
            output: None,
        }
    }
}

impl<Wr, Part> FusedFuture for Send<'_, Wr, Part>
where
    Wr: FusedMultipartWrite<Part> + Unpin,
{
    fn is_terminated(&self) -> bool {
        self.feed.is_terminated()
    }
}

impl<Wr: ?Sized + MultipartWrite<Part> + Unpin, Part> Future for Send<'_, Wr, Part> {
    type Output = Result<Wr::Ret, Wr::Error>;

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
