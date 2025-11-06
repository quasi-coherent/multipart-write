use crate::{FusedMultipartWrite, MultipartWrite};

use futures_core::ready;
use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project_lite::pin_project! {
    /// `MultipartWrite` for [`then`](super::MultipartWriteExt::then).
    #[must_use = "futures do nothing unless polled"]
    pub struct Then<Wr, Fut, F> {
        #[pin]
        writer: Wr,
        #[pin]
        future: Option<Fut>,
        f: F,
    }
}

impl<Wr, Fut, F> Then<Wr, Fut, F> {
    pub(super) fn new(writer: Wr, f: F) -> Self {
        Self {
            writer,
            future: None,
            f,
        }
    }

    /// Consumes `Then`, returning the underlying writer.
    pub fn into_inner(self) -> Wr {
        self.writer
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

impl<Wr, Fut, F, Part, T, E> FusedMultipartWrite<Part> for Then<Wr, Fut, F>
where
    Wr: FusedMultipartWrite<Part>,
    F: FnMut(Result<Wr::Output, Wr::Error>) -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: From<Wr::Error>,
{
    fn is_terminated(&self) -> bool {
        self.writer.is_terminated()
    }
}

impl<Wr, Fut, F, Part, T, E> MultipartWrite<Part> for Then<Wr, Fut, F>
where
    Wr: MultipartWrite<Part>,
    F: FnMut(Result<Wr::Output, Wr::Error>) -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: From<Wr::Error>,
{
    type Ret = Wr::Ret;
    type Output = T;
    type Error = E;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.project().writer.poll_ready(cx))?;
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, part: Part) -> Result<Self::Ret, Self::Error> {
        Ok(self.project().writer.start_send(part)?)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.project().writer.poll_flush(cx))?;
        Poll::Ready(Ok(()))
    }

    fn poll_complete(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        let mut this = self.project();
        if this.future.is_none() {
            let res = ready!(this.writer.poll_complete(cx));
            let fut = (this.f)(res);
            this.future.set(Some(fut));
        }
        let fut = this
            .future
            .as_mut()
            .as_pin_mut()
            .expect("polled Then after completion");
        let out = ready!(fut.poll(cx));
        this.future.set(None);
        Poll::Ready(out)
    }
}

impl<Wr, Fut, F> Debug for Then<Wr, Fut, F>
where
    Wr: Debug,
    Fut: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Then")
            .field("writer", &self.writer)
            .field("future", &self.future)
            .field("f", &"impl FnMut(Result<Wr::Output, Wr::Error>) -> Fut")
            .finish()
    }
}
