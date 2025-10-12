use crate::MultipartWrite;

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
    writer: StreamWriter<W, St::Item>,
    freeze: bool,
    terminated: bool,
}

impl<St: Stream, W: MultipartWrite<St::Item>> CollectParts<St, W> {
    pub(super) fn new(stream: St, writer: W) -> Self {
        Self {
            stream,
            writer: StreamWriter::new(writer),
            freeze: false,
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

        if *this.freeze {
            let ret = task::ready!(this.writer.as_mut().poll_freeze(cx));
            *this.terminated = true;
            return Poll::Ready(ret);
        }

        loop {
            match task::ready!(this.writer.as_mut().poll_write(cx)) {
                Err(e) => {
                    return {
                        *this.terminated = true;
                        Poll::Ready(Err(e))
                    };
                }
                Ok(None) => {
                    let Some(item) = task::ready!(this.stream.as_mut().poll_next(cx)) else {
                        *this.freeze = true;
                        return Poll::Pending;
                    };

                    this.writer.as_mut().set_buffered(item);
                }
                // Nothing to do--the buffered part was written.
                Ok(Some(_)) => {}
            }
        }
    }
}

#[derive(Debug)]
#[pin_project::pin_project]
struct StreamWriter<W, P> {
    #[pin]
    writer: W,
    buffered: Option<P>,
}

impl<W: MultipartWrite<P>, P> StreamWriter<W, P> {
    fn new(writer: W) -> Self {
        Self {
            writer,
            buffered: None,
        }
    }

    fn set_buffered(self: Pin<&mut Self>, part: P) {
        *self.project().buffered = Some(part);
    }

    fn poll_freeze(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<W::Output, W::Error>> {
        let mut this = self.project();
        task::ready!(this.writer.as_mut().poll_flush(cx))?;
        this.writer.as_mut().poll_freeze(cx)
    }

    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Option<W::Ret>, W::Error>> {
        let mut this = self.project();

        if this.buffered.is_none() {
            Poll::Ready(Ok(None))
        } else {
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
                        .expect("polled CollectParts after completion");
                    let ret = this.writer.as_mut().start_write(part)?;
                    Poll::Ready(Ok(Some(ret)))
                }
                Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            }
        }
    }
}
