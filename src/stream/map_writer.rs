use crate::MultipartWrite;

use futures::ready;
use futures::stream::{FusedStream, Stream};
use std::pin::Pin;
use std::task::{Context, Poll};

/// Stream for [`map_writer`](super::MultipartStreamExt::map_writer).
#[derive(Debug)]
#[pin_project::pin_project]
pub struct MapWriter<St: Stream, W: MultipartWrite<St::Item>, F> {
    #[pin]
    stream: St,
    #[pin]
    writer: WriteBuf<W, St::Item, F>,
    state: State,
    is_terminated: bool,
}

impl<St: Stream, W: MultipartWrite<St::Item>, F> MapWriter<St, W, F> {
    pub(super) fn new(stream: St, writer: W, f: F) -> Self {
        Self {
            stream,
            writer: WriteBuf::new(writer, f),
            state: State::Write,
            is_terminated: false,
        }
    }
}

impl<St, W, F> FusedStream for MapWriter<St, W, F>
where
    St: Stream,
    W: MultipartWrite<St::Item>,
    F: FnMut(W::Ret) -> bool,
{
    fn is_terminated(&self) -> bool {
        self.is_terminated
    }
}

impl<St, W, F> Stream for MapWriter<St, W, F>
where
    St: Stream,
    W: MultipartWrite<St::Item>,
    F: FnMut(W::Ret) -> bool,
{
    type Item = Result<W::Output, W::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        loop {
            let next_state = match *this.state {
                // Try to make the writer write its buffered item.
                // * Doesn't have a buffered input, return `State::Next` to poll
                // the stream for the next item.
                // * Returned `Some(true)`, which implies `poll_complete`.
                // * Returned `Some(false)`, can write more items.
                // * Error -- return the error.
                State::Write => match ready!(this.writer.as_mut().poll_writer_send(cx)) {
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
                // writer is not empty, just end here otherwise.
                State::Next => match ready!(this.stream.as_mut().poll_next(cx)) {
                    Some(next) => {
                        this.writer.as_mut().set_buffered(next);
                        State::Write
                    }
                    _ if this.writer.as_mut().is_empty() => return Poll::Ready(None),
                    _ => State::Shutdown(false),
                },
                // Produce the next item downstream.
                State::Complete => {
                    let output = ready!(this.writer.as_mut().poll_writer_complete(cx))?;
                    *this.state = State::Write;
                    return Poll::Ready(Some(Ok(output)));
                }
                // We got here because upstream stopped producing.
                // Either we need to return one more item, or we did that the
                // last iteration and we can end.
                State::Shutdown(now) => {
                    if now {
                        *this.is_terminated = true;
                        return Poll::Ready(None);
                    }
                    let output = ready!(this.writer.as_mut().poll_writer_complete(cx))?;
                    *this.state = State::Shutdown(true);
                    return Poll::Ready(Some(Ok(output)));
                }
            };

            *this.state = next_state;
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum State {
    Write,
    Next,
    Complete,
    Shutdown(bool),
}

#[pin_project::pin_project]
struct WriteBuf<W, T, F> {
    #[pin]
    writer: W,
    buffered: Option<T>,
    f: F,
    is_empty: bool,
}

impl<W, T, F> WriteBuf<W, T, F> {
    fn new(writer: W, f: F) -> Self {
        Self {
            writer,
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

    fn poll_writer_complete(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<W::Output, W::Error>>
    where
        W: MultipartWrite<T>,
        F: FnMut(W::Ret) -> bool,
    {
        let mut this = self.project();
        let output = ready!(this.writer.as_mut().poll_complete(cx))?;
        *this.is_empty = true;
        Poll::Ready(Ok(output))
    }

    fn poll_writer_send(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Option<bool>, W::Error>>
    where
        W: MultipartWrite<T>,
        F: FnMut(W::Ret) -> bool,
    {
        let mut this = self.project();

        if this.buffered.is_none() {
            return Poll::Ready(Ok(None));
        }

        match this.writer.as_mut().poll_ready(cx) {
            Poll::Pending => match this.writer.poll_flush(cx) {
                Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                Poll::Pending | Poll::Ready(Ok(())) => Poll::Pending,
            },
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Ready(Ok(())) => {
                let item = this
                    .buffered
                    .take()
                    .expect("polled MapWriter after completion");

                let ret = this.writer.as_mut().start_send(item).map(this.f)?;
                *this.is_empty = false;

                if ret {
                    Poll::Ready(Ok(Some(true)))
                } else {
                    Poll::Ready(Ok(Some(false)))
                }
            }
        }
    }
}

impl<W: std::fmt::Debug, T: std::fmt::Debug, F> std::fmt::Debug for WriteBuf<W, T, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WriteBuf")
            .field("writer", &self.writer)
            .field("buffered", &self.buffered)
            .finish()
    }
}
