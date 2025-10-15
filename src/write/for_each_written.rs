use crate::AutoMultipartWrite;
use crate::write::stream_writer::{StreamWriter, StreamWriterState};

use futures::future::{FusedFuture, Future};
use futures::stream::{FusedStream, Stream};
use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{self, Context, Poll};

/// Future for the [`for_each_written`] method on streams.
///
/// [`for_each_written`]: super::MultipartWriteStreamExt::for_each_written
#[must_use = "futures do nothing unless polled"]
#[pin_project::pin_project]
pub struct ForEachWritten<St: Stream, W: AutoMultipartWrite<St::Item>, F, Fut> {
    #[pin]
    stream: St,
    #[pin]
    writer: StreamWriter<W, St::Item>,
    #[pin]
    fut: Option<Fut>,
    f: F,
    state: StreamWriterState,
    terminated: bool,
}

impl<St: Stream, W: AutoMultipartWrite<St::Item>, F, Fut> ForEachWritten<St, W, F, Fut> {
    pub(super) fn new(stream: St, writer: W, f: F) -> Self {
        Self {
            stream,
            writer: StreamWriter::new(writer),
            fut: None,
            f,
            state: StreamWriterState::default(),
            terminated: false,
        }
    }
}

impl<St, W, F, Fut> FusedFuture for ForEachWritten<St, W, F, Fut>
where
    St: FusedStream,
    W: AutoMultipartWrite<St::Item>,
    F: FnMut(Result<W::Output, W::Error>) -> Fut,
    Fut: Future<Output = ()>,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated() && self.terminated
    }
}

impl<St, W, F, Fut> Future for ForEachWritten<St, W, F, Fut>
where
    St: Stream,
    W: AutoMultipartWrite<St::Item>,
    F: FnMut(Result<W::Output, W::Error>) -> Fut,
    Fut: Future<Output = ()>,
{
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        loop {
            if let Some(fut) = this.fut.as_mut().as_pin_mut() {
                task::ready!(fut.poll(cx));
                this.fut.set(None);
            }

            let next_state = match *this.state {
                StreamWriterState::Write => {
                    match task::ready!(this.writer.as_mut().poll_write_part(cx)) {
                        Ok(state) => state,
                        Err(e) => {
                            let fut = (this.f)(Err(e));
                            this.fut.set(Some(fut));
                            StreamWriterState::Write
                        }
                    }
                }
                StreamWriterState::Next => match task::ready!(this.stream.as_mut().poll_next(cx)) {
                    Some(item) => {
                        this.writer.as_mut().set_buffered(item);
                        StreamWriterState::Write
                    }
                    _ => {
                        if this.writer.as_mut().is_empty() {
                            *this.terminated = true;
                            return Poll::Ready(());
                        }
                        StreamWriterState::Shutdown(false)
                    }
                },
                StreamWriterState::Freeze => {
                    let output = task::ready!(this.writer.as_mut().poll_freeze_output(cx));
                    let fut = (this.f)(output);
                    this.fut.set(Some(fut));
                    StreamWriterState::Write
                }
                StreamWriterState::Shutdown(now) if now => return Poll::Ready(()),
                StreamWriterState::Shutdown(_) => {
                    let output = task::ready!(this.writer.as_mut().poll_freeze_output(cx));
                    let fut = (this.f)(output);
                    this.fut.set(Some(fut));
                    StreamWriterState::Shutdown(true)
                }
            };

            *this.state = next_state;
        }
    }
}

impl<St: Stream, W: AutoMultipartWrite<St::Item>, F, Fut> Debug for ForEachWritten<St, W, F, Fut>
where
    St: Debug,
    St::Item: Debug,
    W: Debug,
    Fut: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("ForEachWritten")
            .field("stream", &self.stream)
            .field("writer", &self.writer)
            .field("fut", &self.fut)
            .field("f", &"FnMut(Result<W::Output, W::Error>) -> Fut<()>")
            .field("state", &self.state)
            .field("terminated", &self.terminated)
            .finish()
    }
}
