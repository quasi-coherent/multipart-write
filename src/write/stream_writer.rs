//! Helper type for using an [`AutoMultipartWrite`] within a stream.
//!
//! It's required to call [`poll_ready`] on a `MultipartWrite` before a call to
//! [`start_write`], but to have something to write, you need to `poll_next` on
//! the stream.  In between these yield points, the original `poll_ready` may no
//! longer be valid.
//!
//! So it's necessary to be able to `poll_ready` and then immediately write the
//! item with no yield in between. This helper wraps a writer with a buffered
//! item and reports back the next state for the stream-and-writer combo.
use crate::AutoMultipartWrite;

use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{self, Context, Poll};

/// For the caller to track state transitions between polling this writer or
/// polling the stream.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StreamWriterState {
    /// Call [`poll_write_part`] on the writer.
    #[default]
    Write,
    /// Poll the stream for the next item.
    Next,
    /// Poll the writer for its output.
    Freeze,
    /// The stream returned `Poll::Ready(None)` indicating there are no more
    /// items.  So this stream should end as well, but not before producing the
    /// next writer output if possible.
    ///
    /// So `State::Shutdown(false)` is a state that represents, "We are going to
    /// shut down but on the next state transition, not now."  At the conclusion
    /// of this phase, `State::Shutdown(false)` becomes `State::Shutdown(true)`
    /// to mean that nothing else should be done and we are safe to simply return
    /// `Poll::Ready(None)` right there.
    Shutdown(bool),
}

/// `StreamWriter` wraps a writer with a buffered `P` from a stream and writes it
/// when ready.
#[pin_project::pin_project]
pub struct StreamWriter<W, P> {
    #[pin]
    writer: W,
    buffered: Option<P>,
    is_empty: bool,
}

impl<W, P> StreamWriter<W, P> {
    /// Create a new [`StreamWriter`] with empty buffer.
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            buffered: None,
            is_empty: true,
        }
    }

    /// Set the buffer on this writer.
    ///
    /// This should be called only immediately after a previous call to
    /// `poll_write_part` returned `StreamWriterState::Next`.
    pub fn set_buffered(self: Pin<&mut Self>, buffered: P) {
        *self.project().buffered = Some(buffered);
    }

    /// Returns whether this writer's buffer is not empty.
    pub fn has_buffered(self: Pin<&mut Self>) -> bool {
        self.buffered.is_some()
    }

    /// Returns whether this writer has had no parts written and has an empty
    /// buffer.
    pub fn is_empty(self: Pin<&mut Self>) -> bool {
        self.is_empty && self.buffered.is_none()
    }

    /// Attempt to write from the buffer.
    ///
    /// If the buffer is empty, this will immediately return `State::Next`, which
    /// indicates that the caller should poll the stream for the next item.
    ///
    /// If the buffer is not empty, the writer will be polled for readiness,
    /// if necessary by calling `poll_flush` until ready, then write the item.
    /// The function `F` applied to the return type of the writer determines the
    /// next state.
    ///
    /// If it returns `true`, we need to freeze the writer; if `false`, get the
    /// next item from the stream.
    pub fn poll_write_part(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<StreamWriterState, W::Error>>
    where
        W: AutoMultipartWrite<P>,
    {
        let mut this = self.project();

        if this.buffered.is_none() {
            return Poll::Ready(Ok(StreamWriterState::Next));
        }

        match this.writer.as_mut().poll_ready(cx) {
            Poll::Pending => match this.writer.poll_flush(cx) {
                Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                Poll::Ready(Ok(())) | Poll::Pending => Poll::Pending,
            },
            Poll::Ready(Ok(())) => {
                // Generally it's really bad to do something like `Option::take`
                // in a future, but this branch is purely synchronous and we
                // actually want `buffered` to be `None` when returning here.
                let part = this
                    .buffered
                    .take()
                    .expect("polled Frozen after completion");

                let ret = match this.writer.as_mut().start_write(part) {
                    Ok(_) if this.writer.should_freeze() => {
                        Poll::Ready(Ok(StreamWriterState::Freeze))
                    }
                    Ok(_) => Poll::Ready(Ok(StreamWriterState::Next)),
                    Err(e) => Poll::Ready(Err(e)),
                };
                *this.is_empty = false;

                ret
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
        }
    }

    /// Freeze the writer and return the output.
    pub fn poll_freeze_output(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<W::Output, W::Error>>
    where
        W: AutoMultipartWrite<P>,
    {
        let mut this = self.project();
        task::ready!(this.writer.as_mut().poll_flush(cx))?;
        let output = task::ready!(this.writer.as_mut().poll_freeze(cx));
        *this.is_empty = true;
        Poll::Ready(output)
    }
}

impl<W, P> Debug for StreamWriter<W, P>
where
    W: Debug,
    P: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("StreamWriter")
            .field("writer", &self.writer)
            .field("buffered", &self.buffered)
            .field("is_empty", &self.is_empty)
            .finish()
    }
}
