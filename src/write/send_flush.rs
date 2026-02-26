use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::future::FusedFuture;
use futures_core::ready;

use crate::write::Feed;
use crate::{FusedMultipartWrite, MultipartWrite};

/// Future for [`send_flush`](super::MultipartWriteExt::send_flush).
#[must_use = "futures do nothing unless polled"]
pub struct SendFlush<'a, Wr: ?Sized + MultipartWrite<Part>, Part> {
    feed: Feed<'a, Wr, Part>,
    ret: Option<Wr::Recv>,
}

impl<Wr: ?Sized + MultipartWrite<Part> + Unpin, Part> Unpin
    for SendFlush<'_, Wr, Part>
{
}

impl<'a, Wr: ?Sized + MultipartWrite<Part> + Unpin, Part>
    SendFlush<'a, Wr, Part>
{
    pub(super) fn new(writer: &'a mut Wr, part: Part) -> Self {
        Self { feed: Feed::new(writer, part), ret: None }
    }
}

impl<Wr, Part> FusedFuture for SendFlush<'_, Wr, Part>
where
    Wr: FusedMultipartWrite<Part> + Unpin,
{
    fn is_terminated(&self) -> bool {
        self.feed.is_terminated()
    }
}

impl<Wr: ?Sized + MultipartWrite<Part> + Unpin, Part> Future
    for SendFlush<'_, Wr, Part>
{
    type Output = Result<Wr::Recv, Wr::Error>;

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        let this = &mut *self;

        if this.feed.is_part_pending() {
            let ret = ready!(Pin::new(&mut this.feed).poll(cx))?;
            this.ret = Some(ret);
            debug_assert!(!this.feed.is_part_pending());
        }

        ready!(this.feed.writer_pin_mut().poll_flush(cx))?;
        let ret = this.ret.take().expect("polled SendFlush after completion");

        Poll::Ready(Ok(ret))
    }
}

impl<Wr, Part> Debug for SendFlush<'_, Wr, Part>
where
    Wr: MultipartWrite<Part> + Debug,
    Part: Debug,
    Wr::Recv: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("SendFlush")
            .field("feed", &self.feed)
            .field("ret", &self.ret)
            .finish()
    }
}
