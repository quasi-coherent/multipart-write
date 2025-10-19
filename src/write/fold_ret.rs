use crate::{FusedMultipartWrite, MultipartWrite};

use futures_core::ready;
use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

/// `MultipartWrite` for [`fold_ret`](super::MultipartWriteExt::fold_ret).
#[must_use = "futures do nothing unless polled"]
#[pin_project::pin_project]
pub struct FoldRet<Wr, F, T> {
    #[pin]
    writer: Wr,
    acc: Option<T>,
    f: F,
}

impl<Wr, F, T> FoldRet<Wr, F, T> {
    pub(super) fn new(writer: Wr, id: T, f: F) -> Self {
        Self {
            writer,
            acc: Some(id),
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

impl<Wr, F, T, Part> FusedMultipartWrite<Part> for FoldRet<Wr, F, T>
where
    Wr: FusedMultipartWrite<Part>,
    F: FnMut(T, &Wr::Ret) -> T,
{
    fn is_terminated(&self) -> bool {
        self.writer.is_terminated()
    }
}

impl<Wr, F, T, Part> MultipartWrite<Part> for FoldRet<Wr, F, T>
where
    Wr: MultipartWrite<Part>,
    F: FnMut(T, &Wr::Ret) -> T,
{
    type Ret = Wr::Ret;
    type Output = (T, Wr::Output);
    type Error = Wr::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().writer.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, part: Part) -> Result<Self::Ret, Self::Error> {
        let mut this = self.project();
        let ret = this.writer.as_mut().start_send(part)?;
        let new_acc = this.acc.take().map(|acc| (this.f)(acc, &ret));
        *this.acc = new_acc;
        Ok(ret)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().writer.poll_flush(cx)
    }

    fn poll_complete(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        let mut this = self.project();
        let output = ready!(this.writer.as_mut().poll_complete(cx))?;
        let acc = this.acc.take().expect("polled FoldRet after completion");
        Poll::Ready(Ok((acc, output)))
    }
}

impl<Wr, F, T> Debug for FoldRet<Wr, F, T>
where
    Wr: Debug,
    T: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("FoldRet")
            .field("writer", &self.writer)
            .field("acc", &self.acc)
            .finish()
    }
}
