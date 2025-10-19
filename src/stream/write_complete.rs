use crate::{FusedMultipartWrite, MultipartWrite};

use futures_core::future::{FusedFuture, Future};
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use std::pin::Pin;
use std::task::{Context, Poll};

/// Future for [`write_complete`](super::MultipartStreamExt::write_complete).
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
#[pin_project::pin_project]
pub struct WriteComplete<St: Stream, Wr> {
    #[pin]
    writer: Wr,
    #[pin]
    stream: St,
    buffered: Option<St::Item>,
    is_terminated: bool,
}

impl<St: Stream, Wr> WriteComplete<St, Wr> {
    pub(super) fn new(stream: St, writer: Wr) -> Self {
        Self {
            writer,
            stream,
            buffered: None,
            is_terminated: false,
        }
    }
}

impl<St, Wr> FusedFuture for WriteComplete<St, Wr>
where
    Wr: FusedMultipartWrite<St::Item>,
    St: FusedStream,
{
    fn is_terminated(&self) -> bool {
        self.is_terminated
    }
}

impl<St, Wr> Future for WriteComplete<St, Wr>
where
    Wr: MultipartWrite<St::Item>,
    St: Stream,
{
    type Output = Result<Wr::Output, Wr::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        loop {
            if this.buffered.is_some() {
                ready!(this.writer.as_mut().poll_ready(cx))?;
                let _ = this
                    .writer
                    .as_mut()
                    .start_send(this.buffered.take().unwrap())?;
            }

            match this.stream.as_mut().poll_next(cx) {
                Poll::Ready(Some(it)) => *this.buffered = Some(it),
                Poll::Ready(None) => {
                    let output = ready!(this.writer.poll_complete(cx));
                    *this.is_terminated = true;
                    return Poll::Ready(output);
                }
                Poll::Pending => {
                    ready!(this.writer.poll_flush(cx))?;
                    return Poll::Pending;
                }
            }
        }
    }
}
