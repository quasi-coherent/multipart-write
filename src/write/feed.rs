use crate::{FusedMultipartWrite, MultipartWrite};

use futures_core::future::{FusedFuture, Future};
use futures_core::ready;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Future for [`feed`](super::MultipartWriteExt::feed).
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Feed<'a, Wr: ?Sized, Part> {
    writer: &'a mut Wr,
    buffered: Option<Part>,
}

impl<Wr: ?Sized + Unpin, Part> Unpin for Feed<'_, Wr, Part> {}

impl<'a, Wr: ?Sized + MultipartWrite<Part> + Unpin, Part> Feed<'a, Wr, Part> {
    pub(super) fn new(writer: &'a mut Wr, part: Part) -> Self {
        Self {
            writer,
            buffered: Some(part),
        }
    }

    pub(super) fn writer_pin_mut(&mut self) -> Pin<&mut Wr> {
        Pin::new(self.writer)
    }

    pub(super) fn is_part_pending(&self) -> bool {
        self.buffered.is_some()
    }
}

impl<Wr, Part> FusedFuture for Feed<'_, Wr, Part>
where
    Wr: FusedMultipartWrite<Part> + Unpin,
{
    fn is_terminated(&self) -> bool {
        self.writer.is_terminated()
    }
}

impl<Wr: ?Sized + MultipartWrite<Part> + Unpin, Part> Future for Feed<'_, Wr, Part> {
    type Output = Result<Wr::Ret, Wr::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let mut writer = Pin::new(&mut this.writer);
        ready!(writer.as_mut().poll_ready(cx))?;
        let part = this.buffered.take().expect("polled Feed after completion");
        let ret = writer.as_mut().start_send(part);
        Poll::Ready(ret)
    }
}
