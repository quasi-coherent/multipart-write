use crate::MultipartWrite;

use futures::ready;
use futures::stream::{FusedStream, Stream};
use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

/// The type of value for [`into_multipart_write`].
///
/// [`into_multipart_write`]: super::MultipartStreamExt::into_multipart_write
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
#[pin_project::pin_project]
pub struct IntoMultipartWrite<St: Stream, Wr, F> {
    #[pin]
    stream: St,
    #[pin]
    writer: WriteBuf<Wr, St::Item, F>,
    state: State,
}

#[derive(Debug, Clone, Copy)]
enum State {
    Write,
    Next,
    Complete,
    Shutdown(bool),
}

impl<St: Stream, Wr, F> IntoMultipartWrite<St, Wr, F> {
    pub(super) fn new(stream: St, writer: Wr, f: F) -> Self {
        Self {
            stream,
            writer: WriteBuf::new(writer, f),
            state: State::Write,
        }
    }
}

impl<St, Wr, F> FusedStream for IntoMultipartWrite<St, Wr, F>
where
    St: FusedStream,
    Wr: MultipartWrite<St::Item>,
    F: FnMut(Wr::Ret) -> bool,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

impl<St, Wr, F> Stream for IntoMultipartWrite<St, Wr, F>
where
    St: Stream,
    Wr: MultipartWrite<St::Item>,
    F: FnMut(Wr::Ret) -> bool,
{
    type Item = Result<Wr::Output, Wr::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        loop {
            let next_state = match *this.state {
                // Try to make the writer write its buffered item.
                // Possibilities are:
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
                // writer is not empty; end now otherwise.
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

#[pin_project::pin_project]
struct WriteBuf<Wr, T, F> {
    #[pin]
    writer: Wr,
    buffered: Option<T>,
    f: F,
    is_empty: bool,
}

impl<Wr, T, F> WriteBuf<Wr, T, F> {
    fn new(writer: Wr, f: F) -> Self {
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
    ) -> Poll<Result<Wr::Output, Wr::Error>>
    where
        Wr: MultipartWrite<T>,
        F: FnMut(Wr::Ret) -> bool,
    {
        let mut this = self.project();
        ready!(this.writer.as_mut().poll_flush(cx))?;
        let output = ready!(this.writer.as_mut().poll_complete(cx))?;
        *this.is_empty = true;
        Poll::Ready(Ok(output))
    }

    fn poll_writer_send(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Option<bool>, Wr::Error>>
    where
        Wr: MultipartWrite<T>,
        F: FnMut(Wr::Ret) -> bool,
    {
        let mut this = self.project();

        if this.buffered.is_some() {
            ready!(this.writer.as_mut().poll_ready(cx))?;
            let item = this.buffered.take().unwrap();
            let ret = this.writer.as_mut().start_send(item).map(this.f)?;
            *this.is_empty = false;
            Poll::Ready(Ok(Some(ret)))
        } else {
            Poll::Ready(Ok(None))
        }
    }
}

impl<Wr: Debug, T: Debug, F> Debug for WriteBuf<Wr, T, F> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("WriteBuf")
            .field("writer", &self.writer)
            .field("buffered", &self.buffered)
            .field("is_empty", &self.is_empty)
            .finish()
    }
}
