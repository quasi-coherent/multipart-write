use crate::MultipartWrite;

use futures::future::{FusedFuture, Future};
use futures::ready;
use futures::stream::{Fuse, Stream, StreamExt};
use std::pin::Pin;
use std::task::{Context, Poll};

/// Future for [`collect_complete`](super::MultipartStreamExt::collect_complete).
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
#[pin_project::pin_project(project = CollectCompleteProj)]
pub struct CollectComplete<St, W, Part> {
    #[pin]
    writer: Option<W>,
    #[pin]
    stream: Fuse<St>,
    buffered: Option<Part>,
}

impl<St: Stream, W, Part> CollectComplete<St, W, Part> {
    pub(super) fn new(stream: St, writer: W) -> Self {
        Self {
            writer: Some(writer),
            stream: stream.fuse(),
            buffered: None,
        }
    }
}

impl<St, W, Part> FusedFuture for CollectComplete<St, W, Part>
where
    W: MultipartWrite<Part>,
    St: Stream<Item = Result<Part, W::Error>>,
{
    fn is_terminated(&self) -> bool {
        self.writer.is_none()
    }
}

impl<St, W, Part> Future for CollectComplete<St, W, Part>
where
    W: MultipartWrite<Part>,
    St: Stream<Item = Result<Part, W::Error>>,
{
    type Output = Result<W::Output, W::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let CollectCompleteProj {
            mut writer,
            mut stream,
            buffered,
        } = self.project();
        let mut wr = writer
            .as_mut()
            .as_pin_mut()
            .expect("polled `CollectComplete` after completion");

        loop {
            if buffered.is_some() {
                ready!(wr.as_mut().poll_ready(cx))?;
                let _ = wr.as_mut().start_send(buffered.take().unwrap())?;
            }

            match stream.as_mut().poll_next(cx)? {
                Poll::Ready(Some(it)) => {
                    *buffered = Some(it);
                }
                Poll::Ready(None) => {
                    let output = ready!(wr.poll_complete(cx))?;
                    writer.set(None);
                    return Poll::Ready(Ok(output));
                }
                Poll::Pending => {
                    ready!(wr.poll_flush(cx))?;
                    return Poll::Pending;
                }
            }
        }
    }
}
