use crate::MultipartWrite;

use futures::future::Future;
use futures::ready;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Future for [`feed`](super::MultipartWriteExt::feed).
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Feed<'a, W: ?Sized, Part> {
    writer: &'a mut W,
    buffered: Option<Part>,
}

impl<W: ?Sized + Unpin, Part> Unpin for Feed<'_, W, Part> {}

impl<'a, W: ?Sized + MultipartWrite<Part> + Unpin, Part> Feed<'a, W, Part> {
    /// Create a new [`Feed`] with empty buffer.
    pub(super) fn new(writer: &'a mut W, part: Part) -> Self {
        Self {
            writer,
            buffered: Some(part),
        }
    }

    pub(super) fn writer_pin_mut(&mut self) -> Pin<&mut W> {
        Pin::new(self.writer)
    }

    pub(super) fn is_part_pending(&self) -> bool {
        self.buffered.is_some()
    }
}

impl<W: ?Sized + MultipartWrite<Part> + Unpin, Part> Future for Feed<'_, W, Part> {
    type Output = Result<W::Ret, W::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let mut writer = Pin::new(&mut this.writer);
        ready!(writer.as_mut().poll_ready(cx))?;
        let part = this.buffered.take().expect("polled Feed after completion");
        let ret = writer.as_mut().start_send(part);
        Poll::Ready(ret)
    }
}
