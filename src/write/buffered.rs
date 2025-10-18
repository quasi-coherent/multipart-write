use crate::MultipartWrite;

use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{self, Context, Poll};

/// `MultipartWrite` for the [`buffered`] method.
///
/// [`buffered`]: super::MultipartWriteExt::buffered
#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
#[pin_project::pin_project]
pub struct Buffered<W, Part> {
    #[pin]
    writer: W,
    capacity: usize,
    buf: VecDeque<Part>,
}

impl<W: MultipartWrite<Part>, Part> Buffered<W, Part> {
    pub(super) fn new(writer: W, capacity: usize) -> Self {
        Self {
            writer,
            capacity,
            buf: VecDeque::with_capacity(capacity),
        }
    }

    /// Acquires a reference to the underlying writer.
    pub fn get_ref(&self) -> &W {
        &self.writer
    }

    /// Acquires a mutable reference to the underlying writer.
    ///
    /// It is inadvisable to directly write to the underlying writer.
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    /// Acquires a pinned mutable reference to the underlying writer.
    ///
    /// It is inadvisable to directly write to the underlying writer.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut W> {
        self.project().writer
    }

    fn try_empty_buffer(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), W::Error>> {
        let mut this = self.project();

        task::ready!(this.writer.as_mut().poll_ready(cx))?;
        while let Some(part) = this.buf.pop_front() {
            this.writer.as_mut().start_send(part)?;
            if !this.buf.is_empty() {
                task::ready!(this.writer.as_mut().poll_ready(cx))?;
            }
        }
        Poll::Ready(Ok(()))
    }
}

impl<W, Part> MultipartWrite<Part> for Buffered<W, Part>
where
    W: MultipartWrite<Part>,
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

    fn start_send(self: Pin<&mut Self>, part: Part) -> Result<Self::Ret, Self::Error> {
        if self.capacity == 0 {
            let _ = self.project().writer.start_send(part)?;
        } else {
            self.project().buf.push_back(part);
        }
        Ok(())
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        task::ready!(self.as_mut().try_empty_buffer(cx))?;
        self.project().writer.poll_flush(cx)
    }

    fn poll_complete(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        task::ready!(self.as_mut().try_empty_buffer(cx))?;
        self.project().writer.poll_complete(cx)
    }
}
