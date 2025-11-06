use crate::{FusedMultipartWrite, MultipartWrite};

use futures_core::future::{FusedFuture, Future};
use futures_core::ready;
use futures_core::stream::Stream;
use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project_lite::pin_project! {
    /// Future for [`collect_writer`].
    ///
    /// [`collect_writer`]: super::MultipartStreamExt::collect_writer
    #[must_use = "futures do nothing unless polled"]
    pub struct CollectWriter<St: Stream, Wr> {
        #[pin]
        writer: Wr,
        #[pin]
        stream: Option<St>,
        buffered: Option<St::Item>,
        is_terminated: bool,
    }
}

impl<St: Stream, Wr> CollectWriter<St, Wr> {
    pub(super) fn new(stream: St, writer: Wr) -> Self {
        Self {
            writer,
            stream: Some(stream),
            buffered: None,
            is_terminated: false,
        }
    }
}

impl<St, Wr> FusedFuture for CollectWriter<St, Wr>
where
    St: Stream,
    Wr: FusedMultipartWrite<St::Item>,
{
    fn is_terminated(&self) -> bool {
        self.writer.is_terminated() || self.is_terminated
    }
}

impl<St, Wr> Future for CollectWriter<St, Wr>
where
    St: Stream,
    Wr: MultipartWrite<St::Item>,
{
    type Output = Result<Wr::Output, Wr::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        loop {
            if this.buffered.is_some() {
                loop {
                    match this.writer.as_mut().poll_ready(cx) {
                        Poll::Ready(Ok(())) => break,
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                        Poll::Pending => match this.writer.as_mut().poll_flush(cx)? {
                            Poll::Pending => return Poll::Pending,
                            Poll::Ready(()) => {}
                        },
                    }
                }

                let _ = this
                    .writer
                    .as_mut()
                    .start_send(this.buffered.take().unwrap())?;
            }

            let Some(st) = this.stream.as_mut().as_pin_mut() else {
                let output = ready!(this.writer.as_mut().poll_complete(cx));
                *this.is_terminated = true;
                return Poll::Ready(output);
            };

            match ready!(st.poll_next(cx)) {
                Some(it) => *this.buffered = Some(it),
                None => this.stream.set(None),
            }
        }
    }
}

impl<St, Wr> Debug for CollectWriter<St, Wr>
where
    St: Stream + Debug,
    St::Item: Debug,
    Wr: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("CollectWriter")
            .field("writer", &self.writer)
            .field("stream", &self.stream)
            .field("buffered", &self.buffered)
            .field("is_terminated", &self.is_terminated)
            .finish()
    }
}
