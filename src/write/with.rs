use crate::{FusedMultipartWrite, MultipartWrite};

use futures_core::ready;
use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

/// `MultipartWrite` for [`with`](super::MultipartWriteExt::with).
#[must_use = "futures do nothing unless polled"]
#[pin_project::pin_project]
pub struct With<Wr, Part, U, Fut, F> {
    #[pin]
    writer: Wr,
    f: F,
    #[pin]
    future: Option<Fut>,
    _f: std::marker::PhantomData<fn(U) -> Part>,
}

impl<Wr, Part, U, Fut, F> With<Wr, Part, U, Fut, F>
where
    Wr: MultipartWrite<Part>,
    F: FnMut(U) -> Fut,
    Fut: Future,
{
    pub(super) fn new<E>(writer: Wr, f: F) -> Self
    where
        Fut: Future<Output = Result<Part, E>>,
        E: From<Wr::Error>,
    {
        Self {
            writer,
            f,
            future: None,
            _f: std::marker::PhantomData,
        }
    }

    /// Acquires a reference to the underlying writer.
    pub fn get_ref(&self) -> &Wr {
        &self.writer
    }

    /// Acquires a mutable reference to the underlying writer.
    ///
    /// It is inadvisable to directly write to the underlying writer.
    pub fn get_mut(&mut self) -> &mut Wr {
        &mut self.writer
    }

    /// Acquires a pinned mutable reference to the underlying writer.
    ///
    /// It is inadvisable to directly write to the underlying writer.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut Wr> {
        self.project().writer
    }
}

impl<Wr, Part, U, Fut, F, E> With<Wr, Part, U, Fut, F>
where
    Wr: MultipartWrite<Part>,
    F: FnMut(U) -> Fut,
    Fut: Future<Output = Result<Part, E>>,
    E: From<Wr::Error>,
{
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), E>> {
        let mut this = self.project();

        let part = match this.future.as_mut().as_pin_mut() {
            None => return Poll::Ready(Ok(())),
            Some(fut) => ready!(fut.poll(cx))?,
        };
        this.future.set(None);
        this.writer.start_send(part)?;
        Poll::Ready(Ok(()))
    }
}

impl<Wr, Part, U, Fut, F, E> FusedMultipartWrite<U> for With<Wr, Part, U, Fut, F>
where
    Wr: FusedMultipartWrite<Part>,
    F: FnMut(U) -> Fut,
    Fut: Future<Output = Result<Part, E>>,
    E: From<Wr::Error>,
{
    fn is_terminated(&self) -> bool {
        self.writer.is_terminated()
    }
}

impl<Wr, Part, U, Fut, F, E> MultipartWrite<U> for With<Wr, Part, U, Fut, F>
where
    Wr: MultipartWrite<Part>,
    F: FnMut(U) -> Fut,
    Fut: Future<Output = Result<Part, E>>,
    E: From<Wr::Error>,
{
    type Ret = ();
    type Output = Wr::Output;
    type Error = E;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.as_mut().poll(cx))?;
        ready!(self.project().writer.poll_ready(cx)?);
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, part: U) -> Result<Self::Ret, Self::Error> {
        let mut this = self.project();
        this.future.set(Some((this.f)(part)));
        Ok(())
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.as_mut().poll(cx))?;
        ready!(self.project().writer.poll_flush(cx)?);
        Poll::Ready(Ok(()))
    }

    fn poll_complete(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        ready!(self.as_mut().poll(cx))?;
        let ret = ready!(self.project().writer.poll_complete(cx)?);
        Poll::Ready(Ok(ret))
    }
}

impl<Wr, Part, U, Fut, F> Debug for With<Wr, Part, U, Fut, F>
where
    Wr: Debug,
    Fut: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("With")
            .field("writer", &self.writer)
            .field("future", &self.future)
            .finish()
    }
}
