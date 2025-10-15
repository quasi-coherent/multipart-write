use crate::{AutoMultipartWrite, MultipartWrite};

use std::pin::Pin;
use std::task::{self, Context, Poll};

/// `AutoMultipartWrite` for the [`freeze_when`] method.
///
/// [`freeze_when`]: super::MultipartWriteExt::freeze_when
#[must_use = "futures do nothing unless polled"]
#[pin_project::pin_project]
pub struct FreezeWhen<W, S, F> {
    #[pin]
    writer: W,
    state: S,
    f: F,
    should_freeze: bool,
}

impl<W, S: Default, F> FreezeWhen<W, S, F> {
    pub(super) fn new(writer: W, init: S, f: F) -> Self {
        Self {
            writer,
            state: init,
            f,
            should_freeze: false,
        }
    }

    /// Acquires a mutable reference to the underlying writer.
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    /// Acquires a pinned mutable reference to the underlying writer.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut W> {
        self.project().writer
    }
}

impl<W, S, F, P> MultipartWrite<P> for FreezeWhen<W, S, F>
where
    W: MultipartWrite<P>,
    S: Default,
    F: FnMut(&mut S, &W::Ret) -> bool,
{
    type Ret = W::Ret;
    type Output = W::Output;
    type Error = W::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().writer.poll_ready(cx)
    }

    fn start_write(self: Pin<&mut Self>, part: P) -> Result<Self::Ret, Self::Error> {
        let this = self.project();
        let ret = this.writer.start_write(part)?;
        if (this.f)(this.state, &ret) {
            *this.should_freeze = true;
        }
        Ok(ret)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().writer.poll_flush(cx)
    }

    fn poll_freeze(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        let this = self.project();
        let out = task::ready!(this.writer.poll_freeze(cx));
        *this.should_freeze = false;
        *this.state = S::default();
        Poll::Ready(out)
    }
}

impl<W, S, F, P> AutoMultipartWrite<P> for FreezeWhen<W, S, F>
where
    W: MultipartWrite<P>,
    S: Default,
    F: FnMut(&mut S, &W::Ret) -> bool,
{
    fn should_freeze(self: Pin<&mut Self>) -> bool {
        self.should_freeze
    }
}
