use crate::{FusedMultipartWrite, MultipartWrite};

use futures_core::ready;
use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project_lite::pin_project! {
    /// `MultipartWrite` for [`lift`](super::MultipartWriteExt::lift).
    #[must_use = "futures do nothing unless polled"]
    pub struct Lift<Wr, U, Part> {
        #[pin]
        inner: Wr,
        #[pin]
        writer: U,
        buffered: Option<Part>,
    }
}

impl<Wr, U, Part> Lift<Wr, U, Part> {
    pub(super) fn new(inner: Wr, writer: U) -> Self {
        Self {
            inner,
            writer,
            buffered: None,
        }
    }

    fn poll_send_inner<T>(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Wr::Error>>
    where
        U: MultipartWrite<T, Output = Part>,
        Wr: MultipartWrite<Part>,
        Wr::Error: From<U::Error>,
    {
        let mut this = self.project();

        if this.buffered.is_none() {
            let part = ready!(this.writer.as_mut().poll_complete(cx))?;
            *this.buffered = Some(part);
        }
        ready!(this.inner.as_mut().poll_ready(cx))?;
        let _ = this
            .inner
            .as_mut()
            .start_send(this.buffered.take().unwrap())?;

        Poll::Ready(Ok(()))
    }
}

impl<T, Wr, U, Part> FusedMultipartWrite<T> for Lift<Wr, U, Part>
where
    U: FusedMultipartWrite<T, Output = Part>,
    Wr: FusedMultipartWrite<Part>,
    Wr::Error: From<U::Error>,
{
    fn is_terminated(&self) -> bool {
        self.inner.is_terminated() || self.writer.is_terminated()
    }
}

impl<T, Wr, U, Part> MultipartWrite<T> for Lift<Wr, U, Part>
where
    U: MultipartWrite<T, Output = Part>,
    Wr: MultipartWrite<Part>,
    Wr::Error: From<U::Error>,
{
    type Ret = U::Ret;
    type Error = Wr::Error;
    type Output = Wr::Output;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project()
            .writer
            .as_mut()
            .poll_ready(cx)
            .map_err(Wr::Error::from)
    }

    fn start_send(self: Pin<&mut Self>, part: T) -> Result<Self::Ret, Self::Error> {
        self.project()
            .writer
            .as_mut()
            .start_send(part)
            .map_err(Wr::Error::from)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.as_mut().poll_send_inner(cx))?;
        self.project().inner.poll_flush(cx)
    }

    fn poll_complete(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        ready!(self.as_mut().poll_send_inner(cx))?;
        self.project().inner.poll_complete(cx)
    }
}

impl<Wr, U, Part> Debug for Lift<Wr, U, Part>
where
    Wr: Debug,
    U: Debug,
    Part: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Lift")
            .field("inner", &self.inner)
            .field("writer", &self.writer)
            .field("buffered", &self.buffered)
            .finish()
    }
}
