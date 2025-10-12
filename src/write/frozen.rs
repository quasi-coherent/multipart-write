use crate::MultipartWrite;

use futures::stream::{FusedStream, Stream};
use std::pin::Pin;
use std::task::{self, Context, Poll};

/// `MultipartWrite` for the [`frozen`] method on `Stream`s.
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

        match *this.state {
            State::Freeze(shutdown) => {
                let ret = task::ready!(this.writer.as_mut().freeze(cx));
                if shutdown {
                    *this.state = State::Shutdown;
                } else {
                    *this.state = State::Next;
                }

                return Poll::Ready(Some(ret));
            }
            State::Shutdown => return Poll::Ready(None),
            State::Next => {}
        }

        loop {
            match task::ready!(this.writer.as_mut().write_part(cx)) {
                Err(e) => {
                    *this.state = State::Next;
                    return Poll::Ready(Some(Err(e)));
                }
                Ok(State::Next) => {
                    let Some(it) = task::ready!(this.stream.as_mut().poll_next(cx)) else {
                        *this.state = State::Freeze(true);
                        return Poll::Pending;
                    };
                    this.writer.as_mut().set_buffered(it);
                }
                Ok(State::Freeze(_)) => {
                    *this.state = State::Freeze(false);
                    return Poll::Pending;
                }
                _ => {}
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Next,
    Freeze(bool),
    Shutdown,
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
