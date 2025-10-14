use crate::MultipartWrite;
use crate::write::stream_writer::{StreamWriter, StreamWriterState};

use futures::stream::{FusedStream, Stream};
use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{self, Context, Poll};

/// Stream for the [`frozen`] method.
///
/// [`frozen`]: super::MultipartWriteStreamExt::frozen
#[must_use = "futures do nothing unless polled"]
#[pin_project::pin_project]
pub struct Frozen<St: Stream, W: MultipartWrite<St::Item>, F> {
    #[pin]
    stream: St,
    #[pin]
    writer: StreamWriter<W, St::Item, F>,
    state: StreamWriterState,
    terminated: bool,
}

impl<St, W, F> Frozen<St, W, F>
where
    St: Stream,
    W: MultipartWrite<St::Item>,
    F: FnMut(W::Ret) -> bool,
{
    pub(super) fn new(stream: St, writer: W, f: F) -> Self {
        Self {
            stream,
            writer: StreamWriter::new(writer, f),
            state: StreamWriterState::Next,
            terminated: false,
        }
    }
}

impl<St, W, F> FusedStream for Frozen<St, W, F>
where
    St: FusedStream,
    W: MultipartWrite<St::Item>,
    F: FnMut(W::Ret) -> bool,
{
    fn is_terminated(&self) -> bool {
        self.terminated
    }
}

impl<St, W, F> Stream for Frozen<St, W, F>
where
    St: Stream,
    W: MultipartWrite<St::Item>,
    F: FnMut(W::Ret) -> bool,
{
    type Item = Result<W::Output, W::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        // Because `poll_ready` must be called before `start_write`, and to call
        // `start_write` we have to have an item from the stream, there is always
        // a yield point between checking if we can write an item and getting the
        // item to write, which invalidates the `ready` designation of the
        // writer.
        // The common way to solve this is to buffer one item between polls.
        // This creates the following state machine for producing the next item:
        loop {
            let next_state = match *this.state {
                // Ask the writer to write its buffered part, returning the
                // `State` we should transition to.
                // If the writer had no item in its buffer, it will ask for the
                // `State::Next` item from the stream.
                // If it did, the item was written, and then either the writer
                // is full and `State::Freeze`, or it's not and `State::Next`,
                // will be the state.
                StreamWriterState::Write => {
                    match task::ready!(this.writer.as_mut().poll_write_part(cx)) {
                        Ok(state) => state,
                        Err(e) => {
                            *this.state = StreamWriterState::Write;
                            return Poll::Ready(Some(Err(e)));
                        }
                    }
                }
                // The writer has no buffered item to write, so poll the stream
                // to get the next item, set it on the writer, then transition to
                // `StreamWriterState::Write`.
                // If there is no next item, this means the stream is exhausted.
                // If the writer is empty, we just end here.  Otherwise, the next
                // state is `StreamWriterState::Shutdown`, which comprises two
                // iterations:
                // 1. Freeze the last time and return it.
                // 2. End the stream by returning `Poll::Ready(None)`.
                StreamWriterState::Next => match task::ready!(this.stream.as_mut().poll_next(cx)) {
                    Some(item) => {
                        this.writer.as_mut().set_buffered(item);
                        StreamWriterState::Write
                    }
                    _ => {
                        if this.writer.as_mut().is_empty() {
                            *this.terminated = true;
                            return Poll::Ready(None);
                        }

                        StreamWriterState::Shutdown(false)
                    }
                },
                // The writer's `poll_freeze` should be polled and a new write
                // should start.
                StreamWriterState::Freeze => {
                    let output = task::ready!(this.writer.as_mut().poll_freeze_output(cx));
                    *this.state = StreamWriterState::Write;
                    return Poll::Ready(Some(output));
                }
                // Got the signal to end the stream, either `now` if this is the
                // second loop of `StreamWriterState::Shutdown`, or freeze the
                // last output and keep the shutdown state, but with `now` equal
                // to `true`.
                StreamWriterState::Shutdown(now) => {
                    if now {
                        *this.terminated = true;
                        return Poll::Ready(None);
                    }

                    let output = task::ready!(this.writer.as_mut().poll_freeze_output(cx));
                    *this.state = StreamWriterState::Shutdown(true);
                    return Poll::Ready(Some(output));
                }
            };

            *this.state = next_state;
        }
    }
}

impl<St: Stream, W: MultipartWrite<St::Item>, F> Debug for Frozen<St, W, F>
where
    St: Debug,
    St::Item: Debug,
    W: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Frozen")
            .field("stream", &self.stream)
            .field("writer", &self.writer)
            .field("state", &self.state)
            .field("terminated", &self.terminated)
            .finish()
    }
}
