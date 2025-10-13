use crate::MultipartWrite;

use futures::stream::{FusedStream, Stream};
use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{self, Context, Poll};

/// `MultipartWrite` for the [`frozen`] method on streams.
///
/// [`frozen`]: super::MultipartWriteStreamExt::frozen
#[must_use = "futures do nothing unless polled"]
#[pin_project::pin_project]
pub struct Frozen<St: Stream, W: MultipartWrite<St::Item>, F> {
    #[pin]
    stream: St,
    #[pin]
    writer: StreamWriter<W, St::Item, F>,
    state: State,
    terminated: bool,
}

// To track state transitions between polling the writer or stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Write,
    Next,
    Freeze,
    Shutdown(bool),
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
            state: State::Next,
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
                State::Write => match task::ready!(this.writer.as_mut().write_part(cx)) {
                    Ok(state) => state,
                    Err(e) => {
                        *this.state = State::Write;
                        return Poll::Ready(Some(Err(e)));
                    }
                },
                // The writer has no buffered item to write, so poll the stream
                // to get the next item, set it on the writer, then transition to
                // `State::Write`.
                // If there is no next item, this means the stream is exhausted.
                // If the writer is empty, we just end here.  Otherwise, the next
                // state is `State::Shutdown`, which comprises two iterations:
                // one, freeze the last time and return it; two, end the stream
                // by returning `Poll::Ready(None)`.
                State::Next => match task::ready!(this.stream.as_mut().poll_next(cx)) {
                    Some(item) => {
                        this.writer.as_mut().set_buffered(item);
                        State::Write
                    }
                    _ => {
                        if this.writer.as_mut().is_empty() {
                            *this.terminated = true;
                            return Poll::Ready(None);
                        }

                        State::Shutdown(true)
                    }
                },
                // The writer's `poll_freeze` should be polled and a new write
                // should start.
                State::Freeze => {
                    let output = task::ready!(this.writer.as_mut().freeze(cx));
                    *this.state = State::Write;
                    return Poll::Ready(Some(output));
                }
                // Got the signal to end the stream, either `now` if this is the
                // second loop of `State::Shutdown`, or freeze the last output
                // and keep the shutdown state, but with `now` equal to `true`.
                State::Shutdown(now) => {
                    if now {
                        *this.terminated = true;
                        return Poll::Ready(None);
                    }

                    let output = task::ready!(this.writer.as_mut().freeze(cx));
                    *this.state = State::Shutdown(true);
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

#[pin_project::pin_project]
struct StreamWriter<W, P, F> {
    #[pin]
    writer: W,
    buffered: Option<P>,
    f: F,
    is_empty: bool,
}

impl<W, P, F> Debug for StreamWriter<W, P, F>
where
    W: Debug,
    P: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("StreamWriter")
            .field("writer", &self.writer)
            .field("buffered", &self.buffered)
            .field("f", &"FnMut(W::Ret) -> bool")
            .field("is_empty", &self.is_empty)
            .finish()
    }
}

impl<W, P, F> StreamWriter<W, P, F> {
    fn new(writer: W, f: F) -> Self {
        Self {
            writer,
            buffered: None,
            f,
            is_empty: true,
        }
    }

    fn set_buffered(self: Pin<&mut Self>, buffered: P) {
        *self.project().buffered = Some(buffered);
    }

    fn is_empty(self: Pin<&mut Self>) -> bool {
        self.is_empty && self.buffered.is_none()
    }

    fn write_part(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<State, W::Error>>
    where
        W: MultipartWrite<P>,
        F: FnMut(W::Ret) -> bool,
    {
        let mut this = self.project();

        if this.buffered.is_none() {
            return Poll::Ready(Ok(State::Next));
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

                let ret = match this.writer.as_mut().start_write(part).map(|v| (this.f)(v)) {
                    Ok(freeze) if freeze => Poll::Ready(Ok(State::Freeze)),
                    Ok(_) => Poll::Ready(Ok(State::Next)),
                    Err(e) => Poll::Ready(Err(e)),
                };
                *this.is_empty = false;

                ret
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
        }
    }

    fn freeze(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<W::Output, W::Error>>
    where
        W: MultipartWrite<P>,
    {
        let mut this = self.project();
        task::ready!(this.writer.as_mut().poll_flush(cx))?;
        let output = task::ready!(this.writer.as_mut().poll_freeze(cx));
        *this.is_empty = true;
        Poll::Ready(output)
    }
}
