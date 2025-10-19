use crate::{FusedMultipartWrite, MultipartWrite};

use futures_core::ready;
use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project_lite::pin_project! {
    /// `MultipartWrite` for [`then`](super::MultipartWriteExt::then).
    #[must_use = "futures do nothing unless polled"]
    pub struct Then<Wr, F, Fut> {
        #[pin]
        writer: Wr,
        #[pin]
        future: Option<Fut>,
        f: F,
    }
}

impl<Wr, F, Fut> Then<Wr, F, Fut> {
    pub(super) fn new(writer: Wr, f: F) -> Self {
        Self {
            writer,
            future: None,
            f,
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

impl<Wr, F, Fut, Part> FusedMultipartWrite<Part> for Then<Wr, F, Fut>
where
    Wr: FusedMultipartWrite<Part>,
    F: FnMut(Wr::Output) -> Fut,
    Fut: Future,
{
    fn is_terminated(&self) -> bool {
        self.writer.is_terminated()
    }
}

impl<Wr, F, Fut, Part> MultipartWrite<Part> for Then<Wr, F, Fut>
where
    Wr: MultipartWrite<Part>,
    F: FnMut(Wr::Output) -> Fut,
    Fut: Future,
{
    type Ret = Wr::Ret;
    type Output = Fut::Output;
    type Error = Wr::Error;

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

        if this.future.is_none() {
            let ret = ready!(this.writer.poll_complete(cx))?;
            let fut = (this.f)(ret);
            this.future.set(Some(fut));
        }

        let fut = this
            .future
            .as_mut()
            .as_pin_mut()
            .expect("polled Then after completion");
        let ret = ready!(fut.poll(cx));
        this.future.set(None);

        Poll::Ready(Ok(ret))
    }
}

impl<Wr, F, Fut> Debug for Then<Wr, F, Fut>
where
    Wr: Debug,
    Fut: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Then")
            .field("writer", &self.writer)
            .field("future", &self.future)
            .field("f", &"F")
            .finish()
    }
}
