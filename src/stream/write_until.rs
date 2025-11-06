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
    /// [`write_until`]: super::MultipartStreamExt::write_until
    #[must_use = "futures do nothing unless polled"]
    pub struct WriteUntil<St: Stream, Wr, F> {
        #[pin]
        inner: TrySend<St, Wr>,
        f: F,
        state: State,
        is_empty: bool,
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum State {
    PollComplete,
    FinalPollComplete,
    #[default]
    PollNext,
    Terminating,
    Terminated,
}

impl State {
    fn should_complete(self) -> bool {
        self == Self::PollComplete || self.is_last_complete()
    }

    fn is_last_complete(self) -> bool {
        self == Self::FinalPollComplete
    }

    fn is_terminated(self) -> bool {
        self == Self::Terminated
    }
}

impl<St: Stream, Wr, F> WriteUntil<St, Wr, F> {
    pub(super) fn new(inner: St, writer: Wr, f: F) -> Self {
        Self {
            inner: TrySend::new(inner, writer),
            f,
            state: State::PollNext,
            is_empty: true,
        }
    }
}

impl<St, Wr, F> FusedStream for WriteUntil<St, Wr, F>
where
    St: Stream,
    Wr: FusedMultipartWrite<St::Item>,
    F: FnMut(Wr::Ret) -> bool,
{
    fn is_terminated(&self) -> bool {
        self.inner.is_terminated() || self.state.is_terminated()
    }
}

impl<St, Wr, F> Stream for WriteUntil<St, Wr, F>
where
    St: Stream,
    Wr: MultipartWrite<St::Item>,
    F: FnMut(Wr::Ret) -> bool,
{
    type Item = Result<Wr::Output, Wr::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        // Last call to this function was the final `poll_complete` because the
        // source stream stopped producing.  Since we've produced the last output
        // and there is no more to write, we stop producing.
        if *this.state == State::Terminating {
            *this.state = State::Terminated;
            return Poll::Ready(None);
        }

        loop {
            if this.state.should_complete() {
                let res = ready!(this.inner.as_mut().poll_complete(cx));
                if this.state.is_last_complete() {
                    *this.state = State::Terminating;
                } else {
                    *this.state = State::PollNext;
                }
                *this.is_empty = true;
                return Poll::Ready(Some(res));
            }

            match ready!(this.inner.as_mut().poll_next(cx)) {
                Some(Ok(ret)) => {
                    if (this.f)(ret) {
                        *this.state = State::PollComplete;
                    }
                    *this.is_empty = false;
                }
                Some(Err(e)) => return Poll::Ready(Some(Err(e))),
                None => {
                    // If the very last thing we did was `poll_complete`, we
                    // don't need to do it again since nothing has been written,
                    // so shortcut and return `Poll::Ready(None)`.
                    if *this.is_empty {
                        *this.state = State::Terminated;
                        return Poll::Ready(None);
                    }
                    *this.state = State::FinalPollComplete;
                }
            }
        }
    }
}

impl<St: Stream, Wr, F> Debug for WriteUntil<St, Wr, F>
where
    St: Debug,
    St::Item: Debug,
    Wr: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("WriteUntil")
            .field("inner", &self.inner)
            .field("f", &"FnMut(Wr::Ret) -> bool")
            .field("state", &self.state)
            .field("is_empty", &self.is_empty)
            .finish()
    }
}
