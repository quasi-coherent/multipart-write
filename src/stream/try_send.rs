use crate::{FusedMultipartWrite, MultipartWrite};

use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project_lite::pin_project! {
    /// Stream for [`try_send`].
    ///
    /// [`try_send`]: super::MultipartStreamExt::try_send
    #[must_use = "futures do nothing unless polled"]
    pub struct TrySend<St: Stream, Wr> {
        #[pin]
        stream: Option<St>,
        #[pin]
        writer: Wr,
        buffered: Option<St::Item>,
        is_terminated: bool,
    }
}

impl<St: Stream, Wr> TrySend<St, Wr> {
    pub(super) fn new(stream: St, writer: Wr) -> Self {
        Self {
            stream: Some(stream),
            writer,
            buffered: None,
            is_terminated: false,
        }
    }

    pub(super) fn poll_complete(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Wr::Output, Wr::Error>>
    where
        Wr: MultipartWrite<St::Item>,
    {
        let mut this = self.project();
        if this.buffered.is_some() {
            ready!(this.writer.as_mut().poll_ready(cx))?;
            let _ = this
                .writer
                .as_mut()
                .start_send(this.buffered.take().unwrap())?;
        }
        ready!(this.writer.as_mut().poll_flush(cx))?;
        this.writer.as_mut().poll_complete(cx)
    }
}

impl<St, Wr> FusedStream for TrySend<St, Wr>
where
    St: Stream,
    Wr: FusedMultipartWrite<St::Item>,
{
    fn is_terminated(&self) -> bool {
        self.writer.is_terminated() || self.is_terminated
    }
}

impl<St, Wr> Stream for TrySend<St, Wr>
where
    St: Stream,
    Wr: MultipartWrite<St::Item>,
{
    type Item = Result<Wr::Ret, Wr::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        loop {
            if this.buffered.is_some() {
                loop {
                    match this.writer.as_mut().poll_ready(cx) {
                        Poll::Ready(Ok(())) => break,
                        Poll::Ready(Err(e)) => return Poll::Ready(Some(Err(e))),
                        Poll::Pending => match this.writer.as_mut().poll_flush(cx)? {
                            Poll::Pending => return Poll::Pending,
                            Poll::Ready(()) => {}
                        },
                    }
                }

                let ret = this
                    .writer
                    .as_mut()
                    .start_send(this.buffered.take().unwrap())?;
                return Poll::Ready(Some(Ok(ret)));
            }

            let Some(st) = this.stream.as_mut().as_pin_mut() else {
                ready!(this.writer.as_mut().poll_flush(cx))?;
                *this.is_terminated = true;
                return Poll::Ready(None);
            };

            match ready!(st.poll_next(cx)) {
                Some(it) => *this.buffered = Some(it),
                None => this.stream.set(None),
            }
        }
    }
}

impl<St: Stream, Wr> Debug for TrySend<St, Wr>
where
    St::Item: Debug,
    St: Debug,
    Wr: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("TrySend")
            .field("stream", &self.stream)
            .field("writer", &self.writer)
            .field("buffered", &self.buffered)
            .field("is_terminated", &self.is_terminated)
            .finish()
    }
}
