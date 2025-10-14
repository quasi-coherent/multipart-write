use crate::MultipartWrite;
use crate::write::stream_writer::{StreamWriter, StreamWriterState};

use futures::future::{FusedFuture, Future};
use futures::stream::Stream;
use std::pin::Pin;
use std::task::{self, Context, Poll};

/// Future for the [`collect_parts`] method on streams.
///
/// [`collect_parts`]: super::MultipartWriteStreamExt::collect_parts
#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
#[pin_project::pin_project]
pub struct CollectParts<St: Stream, W: MultipartWrite<St::Item>> {
    #[pin]
    stream: St,
    #[pin]
    writer: StreamWriter<W, St::Item, fn(W::Ret) -> bool>,
    state: StreamWriterState,
    terminated: bool,
}

impl<St: Stream, W: MultipartWrite<St::Item>> CollectParts<St, W> {
    pub(super) fn new(stream: St, writer: W) -> Self {
        Self {
            stream,
            writer: StreamWriter::new(writer, |_| false),
            state: StreamWriterState::Next,
            terminated: false,
        }
    }
}

impl<St: Stream, W: MultipartWrite<St::Item>> FusedFuture for CollectParts<St, W> {
    fn is_terminated(&self) -> bool {
        self.terminated
    }
}

impl<St: Stream, W: MultipartWrite<St::Item>> Future for CollectParts<St, W> {
    type Output = Result<W::Output, W::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        loop {
            let next_state = match *this.state {
                StreamWriterState::Write => {
                    match task::ready!(this.writer.as_mut().poll_write_part(cx)) {
                        Ok(_) => StreamWriterState::Next,
                        Err(e) => {
                            *this.terminated = true;
                            return Poll::Ready(Err(e));
                        }
                    }
                }
                StreamWriterState::Next => match task::ready!(this.stream.as_mut().poll_next(cx)) {
                    Some(next) => {
                        this.writer.as_mut().set_buffered(next);
                        StreamWriterState::Write
                    }
                    _ => StreamWriterState::Freeze,
                },
                StreamWriterState::Freeze => {
                    let output = task::ready!(this.writer.as_mut().poll_freeze_output(cx));
                    *this.terminated = true;
                    return Poll::Ready(output);
                }
                _ => *this.state,
            };

            *this.state = next_state;
        }
    }
}
