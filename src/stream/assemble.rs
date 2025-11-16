use crate::MultipartWrite;

use futures_core::future::{FusedFuture, Future};
use futures_core::ready;
use futures_core::stream::Stream;
use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project_lite::pin_project! {
    /// Future for [`assemble`].
    ///
    /// [`assemble`]: super::MultipartStreamExt::assemble
    #[must_use = "futures do nothing unless polled"]
    pub struct Assemble<St: Stream, Wr> {
        #[pin]
        writer: Wr,
        #[pin]
        stream: Option<St>,
        buffered: Option<St::Item>,
        is_terminated: bool,
    }
}

impl<St: Stream, Wr> Assemble<St, Wr> {
    pub(super) fn new(stream: St, writer: Wr) -> Self {
        Self {
            writer,
            stream: Some(stream),
            buffered: None,
            is_terminated: false,
        }
    }
}

impl<St, Wr> FusedFuture for Assemble<St, Wr>
where
    St: Stream,
    Wr: MultipartWrite<St::Item>,
{
    fn is_terminated(&self) -> bool {
        self.is_terminated
    }
}

impl<St, Wr> Future for Assemble<St, Wr>
where
    St: Stream,
    Wr: MultipartWrite<St::Item>,
{
    type Output = Result<Wr::Output, Wr::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        loop {
            if this.buffered.is_some() {
                // Need poll_ready to return ready and immediately send.
                // If poll_ready is not ready, poll_flush until it is.
                if this.writer.as_mut().poll_ready(cx)?.is_ready() {
                    let _ = this
                        .writer
                        .as_mut()
                        .start_send(this.buffered.take().unwrap())?;
                } else {
                    match this.writer.as_mut().poll_flush(cx)? {
                        // Flushed, so back to the top to `poll_ready` again.
                        Poll::Ready(()) => continue,
                        Poll::Pending => return Poll::Pending,
                    }
                }
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

impl<St, Wr> Debug for Assemble<St, Wr>
where
    St: Stream + Debug,
    St::Item: Debug,
    Wr: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Assemble")
            .field("writer", &self.writer)
            .field("stream", &self.stream)
            .field("buffered", &self.buffered)
            .field("is_terminated", &self.is_terminated)
            .finish()
    }
}
