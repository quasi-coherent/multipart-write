use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::ready;

use crate::{FusedMultipartWrite, MultipartWrite};

pin_project_lite::pin_project! {
    /// `MultipartWrite` for [`then`](super::MultipartWriteExt::then).
    #[must_use = "futures do nothing unless polled"]
    pub struct Then<Wr, Part, T, Fut, F> {
        #[pin]
        writer: Wr,
        #[pin]
        fut: Option<Fut>,
        f: F,
        _p: PhantomData<fn(Part) -> T>,
    }
}

impl<Wr, Part, T, Fut, F> Then<Wr, Part, T, Fut, F> {
    pub(super) fn new(writer: Wr, f: F) -> Self {
        Self { writer, fut: None, f, _p: PhantomData }
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

impl<Wr, Part, T, Fut, F> FusedMultipartWrite<Part>
    for Then<Wr, Part, T, Fut, F>
where
    Wr: FusedMultipartWrite<Part>,
    F: FnMut(Result<Wr::Output, Wr::Error>) -> Fut,
    Fut: Future<Output = Result<T, Wr::Error>>,
{
    fn is_terminated(&self) -> bool {
        self.writer.is_terminated()
    }
}

impl<Wr, Part, T, Fut, F> MultipartWrite<Part> for Then<Wr, Part, T, Fut, F>
where
    Wr: MultipartWrite<Part>,
    F: FnMut(Result<Wr::Output, Wr::Error>) -> Fut,
    Fut: Future<Output = Result<T, Wr::Error>>,
{
    type Error = Wr::Error;
    type Output = T;
    type Recv = Wr::Recv;

    fn poll_ready(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.project().writer.poll_ready(cx)
    }

    fn start_send(
        self: Pin<&mut Self>,
        part: Part,
    ) -> Result<Self::Recv, Self::Error> {
        self.project().writer.start_send(part)
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.project().writer.poll_flush(cx)
    }

    fn poll_complete(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        let mut this = self.project();
        if this.fut.is_none() {
            let res = ready!(this.writer.poll_complete(cx));
            let fut = (this.f)(res);
            this.fut.set(Some(fut));
        }
        let fut = this
            .fut
            .as_mut()
            .as_pin_mut()
            .expect("polled Then after completion");
        let out = ready!(fut.poll(cx));
        this.fut.set(None);
        Poll::Ready(out)
    }
}

impl<Wr, Part, T, Fut, F> Debug for Then<Wr, Part, T, Fut, F>
where
    Wr: Debug,
    Fut: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Then")
            .field("writer", &self.writer)
            .field("fut", &self.fut)
            .finish()
    }
}
