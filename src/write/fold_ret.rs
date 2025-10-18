use crate::MultipartWrite;

use futures::ready;
use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

/// `MultipartWrite` for [`fold_ret`](super::MultipartWriteExt::fold_ret).
#[must_use = "futures do nothing unless polled"]
#[pin_project::pin_project]
pub struct FoldRet<W, F, T> {
    #[pin]
    writer: W,
    acc: Option<T>,
    f: F,
}

impl<W, F, T> FoldRet<W, F, T> {
    pub(super) fn new(writer: W, id: T, f: F) -> Self {
        Self {
            writer,
            acc: Some(id),
            f,
        }
    }

    /// Acquires a reference to the underlying writer.
    pub fn get_ref(&self) -> &W {
        &self.writer
    }

    /// Acquires a mutable reference to the underlying writer.
    ///
    /// It is inadvisable to directly write to the underlying writer.
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    /// Acquires a pinned mutable reference to the underlying writer.
    ///
    /// It is inadvisable to directly write to the underlying writer.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut W> {
        self.project().writer
    }
}

impl<W, F, T, Part> MultipartWrite<Part> for FoldRet<W, F, T>
where
    W: MultipartWrite<Part>,
    F: FnMut(T, &W::Ret) -> T,
{
    type Ret = W::Ret;
    type Output = (T, W::Output);
    type Error = W::Error;

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

impl<W, F, T> Debug for FoldRet<W, F, T>
where
    W: Debug,
    T: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("FoldRet")
            .field("writer", &self.writer)
            .field("acc", &self.acc)
            .finish()
    }
}
