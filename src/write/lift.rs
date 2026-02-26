use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::ready;

use crate::{FusedMultipartWrite, MultipartWrite};

pin_project_lite::pin_project! {
    /// `MultipartWrite` for [`lift`](super::MultipartWriteExt::lift).
    #[must_use = "futures do nothing unless polled"]
    pub struct Lift<Wr, U, P, Part> {
        #[pin]
        inner: Wr,
        #[pin]
        writer: U,
        buffered: Option<Part>,
        _p: PhantomData<fn(P)>,
    }
}

impl<Wr, U, P, Part> Lift<Wr, U, P, Part> {
    pub(super) fn new(inner: Wr, writer: U) -> Self {
        Self { inner, writer, buffered: None, _p: PhantomData }
    }

    /// Consumes `Lift`, returning the underlying writer.
    pub fn into_inner(self) -> Wr {
        self.inner
    }

    /// Acquires a reference to the underlying writer.
    pub fn get_ref(&self) -> &Wr {
        &self.inner
    }

    /// Acquires a mutable reference to the underlying writer.
    ///
    /// It is inadvisable to directly write to the underlying writer.
    pub fn get_mut(&mut self) -> &mut Wr {
        &mut self.inner
    }

    /// Acquires a pinned mutable reference to the underlying writer.
    ///
    /// It is inadvisable to directly write to the underlying writer.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut Wr> {
        self.project().inner
    }

    /// Acquires a reference to the outermost writer.
    pub fn get_outer_ref(&self) -> &U {
        &self.writer
    }

    /// Acquires a mutable reference to the outermost writer.
    ///
    /// It is inadvisable to directly write to the outermost writer.
    pub fn get_outer_mut(&mut self) -> &mut U {
        &mut self.writer
    }

    /// Acquires a pinned mutable reference to the outermost writer.
    ///
    /// It is inadvisable to directly write to the outermost writer.
    pub fn get_outer_pin_mut(self: Pin<&mut Self>) -> Pin<&mut U> {
        self.project().writer
    }

    fn poll_send_inner(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Wr::Error>>
    where
        U: MultipartWrite<P, Output = Part>,
        Wr: MultipartWrite<Part>,
        Wr::Error: From<U::Error>,
    {
        let mut this = self.project();

        if this.buffered.is_none() {
            let part = ready!(this.writer.as_mut().poll_complete(cx))?;
            *this.buffered = Some(part);
        }
        ready!(this.inner.as_mut().poll_ready(cx))?;
        let _ =
            this.inner.as_mut().start_send(this.buffered.take().unwrap())?;

        Poll::Ready(Ok(()))
    }
}

impl<Wr, U, P, Part> FusedMultipartWrite<P> for Lift<Wr, U, P, Part>
where
    U: FusedMultipartWrite<P, Output = Part>,
    Wr: FusedMultipartWrite<Part>,
    Wr::Error: From<U::Error>,
{
    fn is_terminated(&self) -> bool {
        self.inner.is_terminated() || self.writer.is_terminated()
    }
}

impl<Wr, U, P, Part> MultipartWrite<P> for Lift<Wr, U, P, Part>
where
    U: MultipartWrite<P, Output = Part>,
    Wr: MultipartWrite<Part>,
    Wr::Error: From<U::Error>,
{
    type Error = Wr::Error;
    type Output = Wr::Output;
    type Recv = U::Recv;

    fn poll_ready(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.project().writer.as_mut().poll_ready(cx).map_err(Wr::Error::from)
    }

    fn start_send(
        self: Pin<&mut Self>,
        part: P,
    ) -> Result<Self::Recv, Self::Error> {
        self.project().writer.as_mut().start_send(part).map_err(Wr::Error::from)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
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

impl<Wr, U, P, Part> Debug for Lift<Wr, U, P, Part>
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
