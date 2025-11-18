use crate::FusedMultipartWrite;

use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project_lite::pin_project! {
    /// Stream for [`assembled`].
    ///
    /// [`assembled`]: super::MultipartStreamExt::assembled
    #[must_use = "futures do nothing unless polled"]
    pub struct Assembled<St: Stream, Wr, F> {
        #[pin]
        stream: St,
        #[pin]
        writer: Wr,
        buffered: Option<St::Item>,
        f: F,
        state: State,
        empty: bool,
        is_terminated: bool,
    }
}

impl<St: Stream, Wr, F> Assembled<St, Wr, F> {
    pub(super) fn new(stream: St, writer: Wr, f: F) -> Self {
        Self {
            stream,
            writer,
            buffered: None,
            f,
            state: State::PollNext,
            empty: true,
            is_terminated: false,
        }
    }
}

impl<St, Wr, F> FusedStream for Assembled<St, Wr, F>
where
    St: Stream,
    Wr: FusedMultipartWrite<St::Item>,
    F: FnMut(&Wr::Ret) -> bool,
{
    fn is_terminated(&self) -> bool {
        self.is_terminated
    }
}

impl<St, Wr, F> Stream for Assembled<St, Wr, F>
where
    St: Stream,
    Wr: FusedMultipartWrite<St::Item>,
    F: FnMut(&Wr::Ret) -> bool,
{
    type Item = Result<Wr::Output, Wr::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        loop {
            // Try to send anything in the buffer first.
            if this.buffered.is_some() {
                match this.writer.as_mut().poll_ready(cx)? {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(()) => {
                        let it = this.buffered.take().unwrap();
                        let ret = this.writer.as_mut().start_send(it)?;
                        *this.empty = false;
                        // Check if we should complete according to `F`.
                        if (this.f)(&ret) {
                            // false since we don't have to shut down the stream
                            // after the `poll_complete`.
                            *this.state = State::PollComplete(false);
                        } else {
                            *this.state = State::PollNext;
                        }
                    }
                }
            }

            match *this.state {
                State::PollNext => match ready!(this.stream.as_mut().poll_next(cx)) {
                    Some(it) => *this.buffered = Some(it),
                    _ => {
                        if *this.empty {
                            // We just completed, so short circuit and end here.
                            *this.is_terminated = true;
                            return Poll::Ready(None);
                        }
                        // The penultimate state when the writer has something to
                        // complete first.
                        *this.state = State::PollComplete(true);
                    }
                },
                State::PollComplete(last) => {
                    let out = ready!(this.writer.as_mut().poll_complete(cx));
                    // Upstream stopped producing in the last iteration, or the
                    // writer indicates through its `FusedMultipartWrite` impl
                    // that it cannot be polled anymore, so set the state to
                    // `Terminated` to immediately produce `None` and end the
                    // stream the next time we are polled.
                    if last || this.writer.is_terminated() {
                        *this.state = State::Terminated;
                    } else {
                        *this.empty = true;
                        *this.state = State::PollNext;
                    }
                    return Poll::Ready(Some(out));
                }
                State::Terminated => {
                    *this.is_terminated = true;
                    return Poll::Ready(None);
                }
            }
        }
    }
}

impl<St, Wr, F> Debug for Assembled<St, Wr, F>
where
    St: Stream + Debug,
    St::Item: Debug,
    St: Debug,
    Wr: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Assembled")
            .field("stream", &self.stream)
            .field("writer", &self.writer)
            .field("buffered", &self.buffered)
            .field("f", &"FnMut(&Wr::Ret) -> bool")
            .field("state", &self.state)
            .field("empty", &self.empty)
            .field("is_terminated", &self.is_terminated)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    PollNext,
    PollComplete(bool),
    Terminated,
}
