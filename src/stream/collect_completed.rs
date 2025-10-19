use crate::{FusedMultipartWrite, MultipartWrite};

use futures::future::{FusedFuture, Future};
use futures::ready;
use futures::stream::{Fuse, Stream, StreamExt};
use std::pin::Pin;
use std::task::{Context, Poll};

/// Future for [`collect_completed`](super::MultipartStreamExt::collect_completed).
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
#[pin_project::pin_project(project = CollectCompletedProj)]
pub struct CollectCompleted<St, Wr, Part> {
    #[pin]
    writer: Option<Wr>,
    #[pin]
    stream: Fuse<St>,
    buffered: Option<Part>,
}

impl<St: Stream, Wr, Part> CollectCompleted<St, Wr, Part> {
    pub(super) fn new(stream: St, writer: Wr) -> Self {
        Self {
            writer: Some(writer),
            stream: stream.fuse(),
            buffered: None,
        }
    }
}

impl<St, Wr, Part> FusedFuture for CollectCompleted<St, Wr, Part>
where
    Wr: FusedMultipartWrite<Part>,
    St: Stream<Item = Result<Part, Wr::Error>>,
{
    fn is_terminated(&self) -> bool {
        self.writer.as_ref().is_none_or(|wr| wr.is_terminated())
    }
}

impl<St, Wr, Part> Future for CollectCompleted<St, Wr, Part>
where
    Wr: MultipartWrite<Part>,
    St: Stream<Item = Result<Part, Wr::Error>>,
{
    type Output = Result<Wr::Output, Wr::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let CollectCompletedProj {
            mut writer,
            mut stream,
            buffered,
        } = self.project();
        let mut wr = writer
            .as_mut()
            .as_pin_mut()
            .expect("polled `CollectCompleted` after completion");

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
