use crate::{FusedMultipartWrite, MultipartWrite};

use futures_core::future::{FusedFuture, Future};
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project_lite::pin_project! {
    /// Future for [`write_complete`](super::MultipartStreamExt::write_complete).
    #[must_use = "futures do nothing unless polled"]
    pub struct WriteComplete<St: Stream, Wr> {
        #[pin]
        writer: Wr,
        #[pin]
        stream: Option<St>,
        buffered: Option<St::Item>,
        is_terminated: bool,
    }
}

impl<St: Stream, Wr> WriteComplete<St, Wr> {
    pub(super) fn new(stream: St, writer: Wr) -> Self {
        Self {
            writer,
            stream: Some(stream),
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
                let Poll::Ready(res) = this.writer.as_mut().poll_ready(cx) else {
                    ready!(this.writer.poll_flush(cx))?;
                    return Poll::Pending;
                };
                match res {
                    Err(e) => return Poll::Ready(Err(e)),
                    Ok(()) => {
                        let _ = this
                            .writer
                            .as_mut()
                            .start_send(this.buffered.take().unwrap())?;
                    }
                }
            }

            let Some(mut st) = this.stream.as_mut().as_pin_mut() else {
                let output = ready!(this.writer.as_mut().poll_complete(cx));
                *this.is_terminated = true;
                return Poll::Ready(output);
            };

            match st.as_mut().poll_next(cx) {
                Poll::Pending => {
                    ready!(this.writer.poll_flush(cx))?;
                    return Poll::Pending;
                }
                Poll::Ready(Some(it)) => *this.buffered = Some(it),
                Poll::Ready(None) => {
                    // Close the stream and start the process to complete
                    // the write/future.
                    this.stream.set(None);
                    match this.writer.as_mut().poll_flush(cx) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(Ok(())) => {
                            let output = ready!(this.writer.as_mut().poll_complete(cx));
                            *this.is_terminated = true;
                            return Poll::Ready(output);
                        }
                        Poll::Ready(Err(e)) => {
                            *this.is_terminated = true;
                            return Poll::Ready(Err(e));
                        }
                    }
                }
            }
        }
    }
}

impl<St, Wr> Debug for WriteComplete<St, Wr>
where
    St: Stream + Debug,
    St::Item: Debug,
    Wr: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("WriteComplete")
            .field("writer", &self.writer)
            .field("stream", &self.stream)
            .field("buffered", &self.buffered)
            .field("is_terminated", &self.is_terminated)
            .finish()
    }
}
