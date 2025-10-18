use crate::MultipartWrite;

use futures::ready;
use std::pin::Pin;
use std::task::{Context, Poll};

/// `MultipartWrite` for [`then`](super::MultipartWriteExt::then).
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

    /// Acquires a reference to the underlying writer.
    pub fn get_ref(&self) -> &W {
        &self.writer
    }

    /// Acquires a mutable reference to the underlying writer.
    ///
    /// It is inadvisable to directly write to the underlying writer.
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    /// Acquires a pinned mutable reference to the underlying writer.
    ///
    /// It is inadvisable to directly write to the underlying writer.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut W> {
        self.project().writer
    }
}

impl<W, F, Fut, Part> MultipartWrite<Part> for Then<W, F, Fut>
where
    W: MultipartWrite<Part>,
    F: FnMut(W::Output) -> Fut,
    Fut: Future,
{
    type Ret = W::Ret;
    type Output = Fut::Output;
    type Error = W::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().writer.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, part: Part) -> Result<Self::Ret, Self::Error> {
        self.project().writer.start_send(part)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().writer.poll_flush(cx)
    }

    fn poll_complete(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        let mut this = self.project();

        if this.output.is_none() {
            let ret = ready!(this.writer.poll_complete(cx))?;
            let fut = (this.f)(ret);
            this.output.set(Some(fut));
        }

        let fut = this
            .output
            .as_mut()
            .as_pin_mut()
            .expect("polled Then after completion");
        let ret = ready!(fut.poll(cx));
        this.output.set(None);

        Poll::Ready(Ok(ret))
    }
}
