use crate::{AutoMultipartWrite, MultipartWrite};

use std::pin::Pin;
use std::task::{self, Context, Poll};

/// `MultipartWrite` for the [`with`] method.
///
/// [`with`]: super::MultipartWriteExt::with
#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
#[pin_project::pin_project]
pub struct With<W, Q, Part, F, Fut> {
    #[pin]
    writer: W,
    #[pin]
    state: Option<Fut>,
    f: F,
    _f: std::marker::PhantomData<fn(Q) -> Part>,
}

impl<W, Q, Part, F, Fut, E> With<W, Q, Part, F, Fut>
where
    W: MultipartWrite<Part>,
    F: FnMut(Q) -> Fut,
    Fut: Future<Output = Result<Part, E>>,
    E: From<W::Error>,
{
    pub(super) fn new(writer: W, f: F) -> Self {
        Self {
            writer,
            state: None,
            f,
            _f: std::marker::PhantomData,
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

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), E>> {
        let mut this = self.project();

        let part = match this.state.as_mut().as_pin_mut() {
            None => return Poll::Ready(Ok(())),
            Some(fut) => task::ready!(fut.poll(cx))?,
        };
        this.state.set(None);
        this.writer.start_write(part)?;
        Poll::Ready(Ok(()))
    }
}

impl<W, Q, Part, F, Fut, E> MultipartWrite<Q> for With<W, Q, Part, F, Fut>
where
    W: MultipartWrite<Part>,
    F: FnMut(Q) -> Fut,
    Fut: Future<Output = Result<Part, E>>,
    E: From<W::Error>,
{
    type Ret = ();
    type Output = W::Output;
    type Error = E;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        task::ready!(self.as_mut().poll(cx))?;
        task::ready!(self.project().writer.poll_ready(cx)?);
        Poll::Ready(Ok(()))
    }

    fn start_write(self: Pin<&mut Self>, part: Q) -> Result<Self::Ret, Self::Error> {
        let mut this = self.project();
        this.state.set(Some((this.f)(part)));
        Ok(())
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        task::ready!(self.as_mut().poll(cx))?;
        task::ready!(self.project().writer.poll_flush(cx)?);
        Poll::Ready(Ok(()))
    }

    fn poll_freeze(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        task::ready!(self.as_mut().poll(cx))?;
        let ret = task::ready!(self.project().writer.poll_freeze(cx)?);
        Poll::Ready(Ok(ret))
    }
}

impl<W, Q, Part, F, Fut, E> AutoMultipartWrite<Q> for With<W, Q, Part, F, Fut>
where
    W: AutoMultipartWrite<Part>,
    F: FnMut(Q) -> Fut,
    Fut: Future<Output = Result<Part, E>>,
    E: From<W::Error>,
{
    fn should_freeze(self: Pin<&mut Self>) -> bool {
        self.project().writer.should_freeze()
    }
}
