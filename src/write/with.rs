use crate::{FusedMultipartWrite, MultipartWrite};

use futures_core::ready;
use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project_lite::pin_project! {
    /// `MultipartWrite` for [`with`](super::MultipartWriteExt::with).
    #[must_use = "futures do nothing unless polled"]
    pub struct With<Wr, Part, Fut, F> {
        #[pin]
        writer: Wr,
        f: F,
        #[pin]
        future: Option<Fut>,
        buffered: Option<Part>,
    }
}

impl<Wr, Part, Fut, F> With<Wr, Part, Fut, F> {
    pub(super) fn new(writer: Wr, f: F) -> Self {
        Self {
            writer,
            f,
            future: None,
            buffered: None,
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

    fn poll<U, E>(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), E>>
    where
        Wr: MultipartWrite<Part>,
        F: FnMut(U) -> Fut,
        Fut: Future<Output = Result<Part, E>>,
        E: From<Wr::Error>,
    {
        let mut this = self.project();

        loop {
            if this.buffered.is_some() {
                // Check if the underlying sink is prepared for another item.
                // If it is, we have to send it without yielding in between.
                match this.writer.as_mut().poll_ready(cx)? {
                    Poll::Ready(()) => {
                        let _ = this.writer.start_send(this.buffered.take().unwrap())?;
                    }
                    Poll::Pending => match this.writer.as_mut().poll_flush(cx)? {
                        Poll::Ready(()) => continue, // check `poll_ready` again
                        Poll::Pending => return Poll::Pending,
                    },
                }
            }
            if let Some(fut) = this.future.as_mut().as_pin_mut() {
                let part = ready!(fut.poll(cx))?;
                *this.buffered = Some(part);
                this.future.set(None);
            }
            return Poll::Ready(Ok(()));
        }
    }
}

impl<Wr, U, E, Part, Fut, F> FusedMultipartWrite<U> for With<Wr, Part, Fut, F>
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

impl<Wr, U, E, Part, Fut, F> MultipartWrite<U> for With<Wr, Part, Fut, F>
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
        let out = ready!(self.project().writer.poll_complete(cx))?;
        Poll::Ready(Ok(out))
    }
}

impl<Wr, Part, Fut, F> Debug for With<Wr, Part, Fut, F>
where
    Wr: Debug,
    Fut: Debug,
    Part: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("With")
            .field("writer", &self.writer)
            .field("f", &"impl FnMut(U) -> Fut")
            .field("future", &self.future)
            .field("buffered", &self.buffered)
            .finish()
    }
}
