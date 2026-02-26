use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::ready;

use crate::{FusedMultipartWrite, MultipartWrite};

pin_project_lite::pin_project! {
    /// `MultipartWrite` for [`fold_sent`].
    ///
    ///[`fold_sent`]: super::MultipartWriteExt::fold_sent
    #[must_use = "futures do nothing unless polled"]
    pub struct FoldSent<Wr, T, F> {
        #[pin]
        writer: Wr,
        acc: Option<T>,
        f: F,
    }
}

impl<Wr, T, F> FoldSent<Wr, T, F> {
    pub(super) fn new(writer: Wr, id: T, f: F) -> Self {
        Self { writer, acc: Some(id), f }
    }

    /// Consumes `FoldSent`, returning the underlying writer.
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

impl<Wr, T, F, Part> FusedMultipartWrite<Part> for FoldSent<Wr, T, F>
where
    Wr: FusedMultipartWrite<Part>,
    F: FnMut(T, &Wr::Recv) -> T,
{
    fn is_terminated(&self) -> bool {
        self.writer.is_terminated()
    }
}

impl<Wr, T, F, Part> MultipartWrite<Part> for FoldSent<Wr, T, F>
where
    Wr: MultipartWrite<Part>,
    F: FnMut(T, &Wr::Recv) -> T,
{
    type Error = Wr::Error;
    type Output = (T, Wr::Output);
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
        let mut this = self.project();
        let ret = this.writer.as_mut().start_send(part)?;
        let new_acc = this.acc.take().map(|acc| (this.f)(acc, &ret));
        *this.acc = new_acc;
        Ok(ret)
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
        let out = ready!(this.writer.as_mut().poll_complete(cx))?;
        let acc = this.acc.take().expect("polled FoldSent after completion");
        Poll::Ready(Ok((acc, out)))
    }
}

impl<Wr, T, F> Debug for FoldSent<Wr, T, F>
where
    Wr: Debug,
    T: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("FoldSent")
            .field("writer", &self.writer)
            .field("acc", &self.acc)
            .finish()
    }
}
