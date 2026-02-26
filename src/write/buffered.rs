use std::collections::VecDeque;
use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::ready;

use crate::{FusedMultipartWrite, MultipartWrite};

pin_project_lite::pin_project! {
    /// `MultipartWrite` for the [`buffered`] method.
    ///
    /// [`buffered`]: super::MultipartWriteExt::buffered
    #[must_use = "futures do nothing unless polled"]
    pub struct Buffered<Wr: MultipartWrite<Part>, Part> {
        #[pin]
        writer: Wr,
        capacity: usize,
        buf: VecDeque<Part>,
        recv: Vec<Wr::Recv>,
    }
}

impl<Part, Wr: MultipartWrite<Part>> Buffered<Wr, Part> {
    pub(super) fn new(writer: Wr, capacity: usize) -> Self {
        Self {
            writer,
            capacity,
            buf: VecDeque::with_capacity(capacity),
            recv: Vec::with_capacity(capacity),
        }
    }

    /// Consumes `Buffered`, returning the underlying writer.
    pub fn into_inner(self) -> Wr {
        self.writer
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

    fn try_empty_buffer(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Wr::Error>> {
        let mut this = self.project();
        // Must check readiness of `Wr`.
        ready!(this.writer.as_mut().poll_ready(cx))?;
        while let Some(part) = this.buf.pop_front() {
            let recv = this.writer.as_mut().start_send(part)?;
            this.recv.push(recv);
            if !this.buf.is_empty() {
                // Check readiness again; if ready we'll stay in the loop.  If
                // not we'll re-enter at the top and that readiness check will
                // cover it.
                ready!(this.writer.as_mut().poll_ready(cx))?;
            }
        }
        Poll::Ready(Ok(()))
    }
}

impl<Part, Wr: FusedMultipartWrite<Part>> FusedMultipartWrite<Part>
    for Buffered<Wr, Part>
{
    fn is_terminated(&self) -> bool {
        self.writer.is_terminated()
    }
}

impl<Part, Wr: MultipartWrite<Part>> MultipartWrite<Part>
    for Buffered<Wr, Part>
{
    type Error = Wr::Error;
    type Output = Wr::Output;
    type Recv = Option<Vec<Wr::Recv>>;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        if self.capacity == 0 {
            return self.project().writer.poll_ready(cx);
        }
        ready!(self.as_mut().try_empty_buffer(cx))?;
        if self.buf.len() >= self.capacity {
            Poll::Pending
        } else {
            Poll::Ready(Ok(()))
        }
    }

    fn start_send(
        self: Pin<&mut Self>,
        part: Part,
    ) -> Result<Self::Recv, Self::Error> {
        if self.capacity == 0 {
            let recv = self.project().writer.start_send(part)?;
            return Ok(Some(vec![recv]));
        }
        let this = self.project();
        this.buf.push_back(part);
        // If we have accumulated enough of the values returned by the inner
        // writer's `start_send` return the vector of them.
        if this.recv.len() >= *this.capacity {
            let new_recv = Vec::with_capacity(*this.capacity);
            let recv = std::mem::replace(this.recv, new_recv);
            Ok(Some(recv))
        } else {
            Ok(None)
        }
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        ready!(self.as_mut().try_empty_buffer(cx))?;
        self.project().writer.poll_flush(cx)
    }

    fn poll_complete(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        ready!(self.as_mut().try_empty_buffer(cx))?;
        self.project().writer.poll_complete(cx)
    }
}

impl<Wr, Part> Debug for Buffered<Wr, Part>
where
    Part: Debug,
    Wr: Debug + MultipartWrite<Part>,
    Wr::Recv: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Buffered")
            .field("writer", &self.writer)
            .field("capacity", &self.capacity)
            .field("buf", &self.buf)
            .field("recv", &self.recv)
            .finish()
    }
}
