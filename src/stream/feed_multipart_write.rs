use crate::{FusedMultipartWrite, MultipartWrite};

use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project_lite::pin_project! {
    /// Stream for  [`feed_multipart_write`].
    ///
    /// [`feed_multipart_write`]: super::MultipartStreamExt::feed_multipart_write
    #[must_use = "futures do nothing unless polled"]
    pub struct FeedMultipartWrite<St: Stream, Wr, F> {
        #[pin]
        stream: St,
        #[pin]
        writer: WriteBuf<Wr, St::Item, F>,
        state: State,
        stream_terminated: bool,
        is_terminated: bool,
    }
}

#[derive(Debug, Clone, Copy)]
enum State {
    Write,
    Next,
    Complete,
    Shutdown,
}

impl<St: Stream, Wr, F> FeedMultipartWrite<St, Wr, F> {
    pub(super) fn new(stream: St, writer: Wr, f: F) -> Self {
        Self {
            stream,
            writer: WriteBuf::new(writer, f),
            state: State::Write,
            stream_terminated: false,
            is_terminated: false,
        }
    }
}

impl<St, Wr, F> FusedStream for FeedMultipartWrite<St, Wr, F>
where
    St: Stream,
    Wr: FusedMultipartWrite<St::Item>,
    F: FnMut(Wr::Ret) -> bool,
{
    fn is_terminated(&self) -> bool {
        self.is_terminated
    }
}

impl<St, Wr, F> Stream for FeedMultipartWrite<St, Wr, F>
where
    St: Stream,
    Wr: FusedMultipartWrite<St::Item>,
    F: FnMut(Wr::Ret) -> bool,
{
    type Item = Result<Wr::Output, Wr::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        loop {
            // We can't make any more progress if this writer is fused so stop
            // producing the stream.
            if this.writer.inner.is_terminated() {
                *this.is_terminated = true;
                return Poll::Ready(None);
            }

            let next_state = match *this.state {
                // Try to make the writer write its buffered item.
                // Possibilities are:
                // * Doesn't have a buffered input, return `State::Next` to poll
                // the stream for the next item.
                // * Returned `Some(true)`, which implies `poll_complete`.
                // * Returned `Some(false)`, can write more items.
                // * Error -- return the error.
                State::Write => match ready!(this.writer.as_mut().poll_inner_send(cx)) {
                    Ok(None) => State::Next,
                    Ok(Some(b)) if b => State::Complete,
                    Ok(_) => State::Write,
                    Err(e) => {
                        *this.state = State::Write;
                        return Poll::Ready(Some(Err(e)));
                    }
                },
                // Poll for the next upstream item.
                // If there is none, then do the last `poll_complete` if the
                // writer is not empty; end now otherwise.
                State::Next => match ready!(this.stream.as_mut().poll_next(cx)) {
                    Some(next) => {
                        this.writer.as_mut().set_buffered(next);
                        State::Write
                    }
                    _ => {
                        *this.stream_terminated = true;
                        if this.writer.as_mut().is_empty() {
                            return Poll::Ready(None);
                        }
                        State::Complete
                    }
                },
                // Produce the next item downstream.
                State::Complete => {
                    let output = ready!(this.writer.as_mut().poll_inner_complete(cx))?;
                    if *this.stream_terminated {
                        *this.state = State::Shutdown;
                    } else {
                        *this.state = State::Write;
                    }
                    return Poll::Ready(Some(Ok(output)));
                }
                State::Shutdown => {
                    *this.is_terminated = true;
                    return Poll::Ready(None);
                }
            };

            *this.state = next_state;
        }
    }
}

impl<St, Wr, F> Debug for FeedMultipartWrite<St, Wr, F>
where
    St: Stream + Debug,
    St::Item: Debug,
    Wr: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeedMultipartWrite")
            .field("stream", &self.stream)
            .field("writer", &self.writer)
            .field("state", &self.state)
            .field("stream_terminated", &self.stream_terminated)
            .field("is_terminated", &self.is_terminated)
            .finish()
    }
}

pin_project_lite::pin_project! {
    struct WriteBuf<Wr, T, F> {
        #[pin]
        inner: Wr,
        buffered: Option<T>,
        f: F,
        is_empty: bool,
    }
}

impl<Wr, T, F> WriteBuf<Wr, T, F> {
    fn new(inner: Wr, f: F) -> Self {
        Self {
            inner,
            buffered: None,
            f,
            is_empty: false,
        }
    }

    fn is_empty(&self) -> bool {
        self.is_empty && self.buffered.is_none()
    }

    fn set_buffered(self: Pin<&mut Self>, t: T) {
        *self.project().buffered = Some(t);
    }

    fn poll_inner_complete(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Wr::Output, Wr::Error>>
    where
        Wr: MultipartWrite<T>,
        F: FnMut(Wr::Ret) -> bool,
    {
        let mut this = self.project();
        let output = ready!(this.inner.as_mut().poll_complete(cx))?;
        *this.is_empty = true;
        Poll::Ready(Ok(output))
    }

    fn poll_inner_send(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Option<bool>, Wr::Error>>
    where
        Wr: MultipartWrite<T>,
        F: FnMut(Wr::Ret) -> bool,
    {
        let mut this = self.project();

        if this.buffered.is_none() {
            Poll::Ready(Ok(None))
        } else {
            // Flush the writer if unavailable.
            let Poll::Ready(res) = this.inner.as_mut().poll_ready(cx) else {
                ready!(this.inner.as_mut().poll_flush(cx))?;
                return Poll::Pending;
            };

            match res {
                Err(e) => Poll::Ready(Err(e)),
                Ok(()) => {
                    let item = this.buffered.take().unwrap();
                    let ret = this.inner.as_mut().start_send(item).map(this.f)?;
                    *this.is_empty = false;
                    Poll::Ready(Ok(Some(ret)))
                }
            }
        }
    }
}

impl<Wr: Debug, T: Debug, F> Debug for WriteBuf<Wr, T, F> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("WriteBuf")
            .field("inner", &self.inner)
            .field("buffered", &self.buffered)
            .field("is_empty", &self.is_empty)
            .finish()
    }
}
