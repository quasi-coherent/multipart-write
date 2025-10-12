use crate::MultipartWrite;

use futures::stream::{FusedStream, Stream};
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
}

// To track state transitions between polling the writer or stream.
#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Next,
    // The next state should be `Self::Shutdown` when the inner bool is `true`,
    // `Self::Next` if `false`.
    Freeze(bool),
    Shutdown,
}

impl<St: Stream, W, F> Frozen<St, W, F>
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
        self.stream.is_terminated() && self.state == State::Shutdown
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
        // item to write.
        // The common way to solve this is to buffer one item between polls.
        // This creates the following state machine for producing the next item:
        match *this.state {
            // Polling the stream on the last iteration produced `None`, so we
            // need to also produce `None`, ending the stream.
            State::Shutdown => return Poll::Ready(None),
            // The state returned by the inner writer says that it was ready to
            // call `poll_freeze` on, or the stream did not produce the next item
            // to give to the writer and we shortcut to freezing, in this case for
            // the last time, since the state is set to `State::Shutdown` to end
            // the stream on the next poll.
            State::Freeze(shutdown) => {
                let ret = task::ready!(this.writer.as_mut().freeze(cx));
                if shutdown {
                    *this.state = State::Shutdown;
                } else {
                    *this.state = State::Next;
                }

                return Poll::Ready(Some(ret));
            }
            // Enter the item-producing loop.
            State::Next => {}
        }

        loop {
            // Ask the writer to write its buffered part, returning the `State`
            // we should transition to.
            match task::ready!(this.writer.as_mut().write_part(cx)) {
                // This was an error; produce the error as the next stream item.
                Err(e) => {
                    *this.state = State::Next;
                    return Poll::Ready(Some(Err(e)));
                }
                // Either the inner writer had a buffered item or it didn't.
                // If it did not, we have to poll the stream for the next one and
                // write it to the writer.
                // If it did, then the writer was able to safely `poll_ready` and
                // `start_write`.  If `State::Next` was returned in this case,
                // that means that the writer needs more parts to be frozen, so
                // go back to the top of the loop.
                Ok(State::Next) => {
                    // Upstream stopped producing; shortcut the writer and advance
                    // to freezing it for the last item before we also stop
                    // producing.
                    let Some(it) = task::ready!(this.stream.as_mut().poll_next(cx)) else {
                        *this.state = State::Freeze(true);
                        return Poll::Pending;
                    };
                    this.writer.as_mut().set_buffered(it);
                }
                // The `W::Ret` that was returned from the write implies we
                // should freeze the writer and return the next item from this
                // stream.  Set the state to this and return pending so that we
                // enter the match statement at the top on the next poll.
                Ok(State::Freeze(_)) => {
                    *this.state = State::Freeze(false);
                    return Poll::Pending;
                }
                _ => {}
            }
        }
    }
}

#[pin_project::pin_project]
struct StreamWriter<W, P, F> {
    #[pin]
    writer: W,
    buffered: Option<P>,
    f: F,
}

impl<W, P, F> StreamWriter<W, P, F> {
    fn new(writer: W, f: F) -> Self {
        Self {
            writer,
            buffered: None,
            f,
        }
    }

    fn set_buffered(self: Pin<&mut Self>, buffered: P) {
        *self.project().buffered = Some(buffered);
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
            Poll::Pending => {
                if let Err(e) = task::ready!(this.writer.poll_flush(cx)) {
                    Poll::Ready(Err(e))
                } else {
                    Poll::Pending
                }
            }
            Poll::Ready(Ok(())) => {
                // Generally it's really bad to do something like `Option::take`
                // in a future, but here it's safe because we aren't returning
                // `Poll::Pending` at any point between now and returning from
                // this function, and we want the buffer to have `None` in it
                // when we return from this branch.
                let part = this
                    .buffered
                    .take()
                    .expect("polled Frozen after completion");

                match this.writer.as_mut().start_write(part).map(|v| (this.f)(v)) {
                    Ok(freeze) if freeze => Poll::Ready(Ok(State::Freeze(false))),
                    Ok(_) => Poll::Ready(Ok(State::Next)),
                    Err(e) => Poll::Ready(Err(e)),
                }
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
        }
    }

    fn freeze(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<W::Output, W::Error>>
    where
        W: MultipartWrite<P>,
    {
        task::ready!(self.as_mut().project().writer.poll_flush(cx))?;
        self.project().writer.as_mut().poll_freeze(cx)
    }
}
