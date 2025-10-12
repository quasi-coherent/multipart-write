use crate::MultipartWrite;

use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{self, Context, Poll};

/// `MultipartWrite` for the [`buffered`] method.
///
/// [`buffered`]: super::MultipartWriteExt::buffered
#[must_use = "futures do nothing unless polled"]
#[pin_project::pin_project]
pub struct Buffered<W, P> {
    #[pin]
    writer: W,
    capacity: usize,
    buf: VecDeque<P>,
}

impl<W: MultipartWrite<P>, P> Buffered<W, P> {
    pub(super) fn new(writer: W, capacity: usize) -> Self {
        Self {
            writer,
            capacity,
            buf: VecDeque::with_capacity(capacity),
        }
    }

    fn try_empty_buffer(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), W::Error>> {
        let mut this = self.project();

        task::ready!(this.writer.as_mut().poll_ready(cx))?;
        while let Some(part) = this.buf.pop_front() {
            this.writer.as_mut().start_write(part)?;
            if !this.buf.is_empty() {
                task::ready!(this.writer.as_mut().poll_ready(cx))?;
            }
        }
        Poll::Ready(Ok(()))
    }
}

impl<W, P> MultipartWrite<P> for Buffered<W, P>
where
    W: MultipartWrite<P>,
{
    type Ret = ();
    type Output = W::Output;
    type Error = W::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.capacity == 0 {
            return self.project().writer.poll_ready(cx);
        }

        let _ = self.as_mut().try_empty_buffer(cx)?;

        if self.buf.len() >= self.capacity {
            Poll::Pending
        } else {
            Poll::Ready(Ok(()))
        }
    }

    fn start_write(self: Pin<&mut Self>, part: P) -> Result<Self::Ret, Self::Error> {
        if self.capacity == 0 {
            let _ = self.project().writer.start_write(part)?;
        } else {
            self.project().buf.push_back(part);
        }

        Ok(())
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        task::ready!(self.as_mut().try_empty_buffer(cx))?;
        self.project().writer.poll_flush(cx)
    }

    fn poll_freeze(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        task::ready!(self.as_mut().try_empty_buffer(cx))?;
        self.project().writer.poll_freeze(cx)
    }
}
