use super::TrySend;
use crate::{FusedMultipartWrite, MultipartWrite};

use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project_lite::pin_project! {
    /// Stream for [`complete_when`].
    ///
    /// [`complete_when`]: super::MultipartStreamExt::complete_when
    #[must_use = "futures do nothing unless polled"]
    pub struct CompleteWhen<St: Stream, Wr, F> {
        #[pin]
        stream: TrySend<St, Wr>,
        f: F,
        should_complete: bool,
        is_terminated: bool,
    }
}

impl<St: Stream, Wr, F> CompleteWhen<St, Wr, F> {
    pub(super) fn new(stream: St, writer: Wr, f: F) -> Self {
        Self {
            stream: TrySend::new(stream, writer),
            f,
            should_complete: false,
            is_terminated: false,
        }
    }
}

impl<St, Wr, F> FusedStream for CompleteWhen<St, Wr, F>
where
    St: Stream,
    Wr: FusedMultipartWrite<St::Item>,
    F: FnMut(Wr::Ret) -> bool,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated() || self.is_terminated
    }
}

impl<St, Wr, F> Stream for CompleteWhen<St, Wr, F>
where
    St: Stream,
    Wr: MultipartWrite<St::Item>,
    F: FnMut(Wr::Ret) -> bool,
{
    type Item = Result<Wr::Output, Wr::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        loop {
            if *this.should_complete {
                let res = ready!(this.stream.as_mut().poll_complete(cx));
                *this.should_complete = false;
                return Poll::Ready(Some(res));
            }

            match ready!(this.stream.as_mut().poll_next(cx)) {
                Some(Ok(res)) => {
                    if (this.f)(res) {
                        *this.should_complete = true;
                    }
                }
                Some(Err(e)) => return Poll::Ready(Some(Err(e))),
                None => {
                    *this.is_terminated = true;
                    return Poll::Ready(None);
                }
            }
        }
    }
}

impl<St: Stream, Wr, F> Debug for CompleteWhen<St, Wr, F>
where
    St: Debug,
    St::Item: Debug,
    Wr: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompleteWhen")
            .field("stream", &self.stream)
            .field("f", &"FnMut(Wr::Ret) -> bool")
            .field("should_complete", &self.should_complete)
            .field("is_terminated", &self.is_terminated)
            .finish()
    }
}
