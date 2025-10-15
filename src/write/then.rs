use crate::{AutoMultipartWrite, MultipartWrite};

use std::pin::Pin;
use std::task::{self, Context, Poll};

/// `MultipartWrite` for the [`then`] method.
///
/// [`then`]: super::MultipartWriteExt::then
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
#[pin_project::pin_project]
pub struct Then<W, F, Fut> {
    #[pin]
    writer: W,
    #[pin]
    output: Option<Fut>,
    f: F,
}

impl<W, F, Fut> Then<W, F, Fut> {
    pub(super) fn new(writer: W, f: F) -> Self {
        Self {
            writer,
            output: None,
            f,
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

impl<W, F, Fut, P> MultipartWrite<P> for Then<W, F, Fut>
where
    W: MultipartWrite<P>,
    F: FnMut(W::Output) -> Fut,
    Fut: Future,
{
    type Ret = W::Ret;
    type Output = Fut::Output;
    type Error = W::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().writer.poll_ready(cx)
    }

    fn start_write(self: Pin<&mut Self>, part: P) -> Result<Self::Ret, Self::Error> {
        self.project().writer.start_write(part)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().writer.poll_flush(cx)
    }

    fn poll_freeze(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        let mut this = self.project();

        if this.output.is_none() {
            let ret = task::ready!(this.writer.poll_freeze(cx))?;
            let fut = (this.f)(ret);
            this.output.set(Some(fut));
        }

        let fut = this
            .output
            .as_mut()
            .as_pin_mut()
            .expect("polled Then after completion");
        let ret = task::ready!(fut.poll(cx));
        this.output.set(None);

        Poll::Ready(Ok(ret))
    }
}

impl<W, F, Fut, P> AutoMultipartWrite<P> for Then<W, F, Fut>
where
    W: AutoMultipartWrite<P>,
    F: FnMut(W::Output) -> Fut,
    Fut: Future,
{
    fn should_freeze(self: Pin<&mut Self>) -> bool {
        self.project().writer.should_freeze()
    }
}
