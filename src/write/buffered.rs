use crate::{FusedMultipartWrite, MultipartWrite};

use std::collections::VecDeque;
use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{self, Context, Poll};

pin_project_lite::pin_project! {
    /// `MultipartWrite` for the [`buffered`] method.
    ///
    /// [`buffered`]: super::MultipartWriteExt::buffered
    #[must_use = "futures do nothing unless polled"]
    pub struct Buffered<Wr, Part> {
        #[pin]
        writer: Wr,
        capacity: usize,
        buf: VecDeque<Part>,
    }
}

impl<Wr: MultipartWrite<Part>, Part> Buffered<Wr, Part> {
    pub(super) fn new(writer: Wr, capacity: usize) -> Self {
        Self {
            writer,
            capacity,
            buf: VecDeque::with_capacity(capacity),
        }
    }

    /// Acquires a reference to the underlying writer.
    pub fn get_ref(&self) -> &Wr {
        &self.writer
    }

    /// Acquires a mutable reference to the underlying writer.
    ///
    /// It is inadvisable to directly write to the underlying writer.
    pub fn get_mut(&mut self) -> &mut Wr {
        &mut self.writer
    }

    /// Acquires a pinned mutable reference to the underlying writer.
    ///
    /// It is inadvisable to directly write to the underlying writer.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut Wr> {
        self.project().writer
    }

    fn try_empty_buffer(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Wr::Error>> {
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

impl<Wr, Part> FusedMultipartWrite<Part> for Buffered<Wr, Part>
where
    Wr: FusedMultipartWrite<Part>,
{
    fn is_terminated(&self) -> bool {
        self.writer.is_terminated()
    }
}

impl<Wr, Part> MultipartWrite<Part> for Buffered<Wr, Part>
where
    Wr: MultipartWrite<Part>,
{
    type Ret = ();
    type Output = Wr::Output;
    type Error = Wr::Error;

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

impl<Wr: Debug, Part: Debug> Debug for Buffered<Wr, Part> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Buffered")
            .field("writer", &self.writer)
            .field("capacity", &self.capacity)
            .field("buf", &self.buf)
            .finish()
    }
}
