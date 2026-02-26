use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::{Future, ready};

use crate::{FusedMultipartWrite, MultipartWrite};

pin_project_lite::pin_project! {
    /// `MultipartWrite` for [`for_each_recv`].
    ///
    /// [`for_each_recv`]: super::MultipartWriteExt::for_each_recv
    #[must_use = "futures do nothing unless polled"]
    pub struct ForEachRecv<Wr, Part, Fut, F> {
        #[pin]
        writer: Wr,
        #[pin]
        fut: Option<Fut>,
        f: F,
        _p: PhantomData<Part>,
    }
}

impl<Wr, Part, Fut, F> ForEachRecv<Wr, Part, Fut, F> {
    pub(super) fn new(writer: Wr, f: F) -> Self {
        Self { writer, fut: None, f, _p: PhantomData }
    }

    /// Consumes `ForEachRecv`, returning the underlying writer.
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

impl<Wr, Part, Fut, F> FusedMultipartWrite<Part>
    for ForEachRecv<Wr, Part, Fut, F>
where
    Wr: FusedMultipartWrite<Part>,
    Wr::Recv: Clone,
    F: FnMut(Wr::Recv) -> Fut,
    Fut: Future<Output = ()>,
{
    fn is_terminated(&self) -> bool {
        self.writer.is_terminated() && self.fut.is_none()
    }
}

impl<Wr, Part, Fut, F> MultipartWrite<Part> for ForEachRecv<Wr, Part, Fut, F>
where
    Wr: MultipartWrite<Part>,
    Wr::Recv: Clone,
    F: FnMut(Wr::Recv) -> Fut,
    Fut: Future<Output = ()>,
{
    type Error = Wr::Error;
    type Output = Wr::Output;
    type Recv = Wr::Recv;

    fn poll_ready(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        let mut this = self.project();
        if let Some(fut) = this.fut.as_mut().as_pin_mut() {
            ready!(fut.poll(cx));
            this.fut.set(None);
        }
        this.writer.poll_ready(cx)
    }

    fn start_send(
        self: Pin<&mut Self>,
        part: Part,
    ) -> Result<Self::Recv, Self::Error> {
        let mut this = self.project();
        let recv = this.writer.as_mut().start_send(part)?;
        let fut = (this.f)(recv.clone());
        this.fut.set(Some(fut));
        Ok(recv)
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
        self.project().writer.poll_complete(cx)
    }
}

impl<Wr, Part, Fut, F> Debug for ForEachRecv<Wr, Part, Fut, F>
where
    Wr: Debug,
    Fut: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("ForEachRecv")
            .field("writer", &self.writer)
            .field("fut", &self.fut)
            .finish()
    }
}
