use crate::{FusedMultipartWrite, MultipartWrite};

use futures_core::{Future, ready};
use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

/// `MultipartWrite` for [`bootstrapped`].
///
/// [`bootstrapped`]: super::MultipartWriteExt::bootstrapped
#[must_use = "futures do nothing unless polled"]
#[pin_project::pin_project]
pub struct Bootstrapped<Wr, S, F, Fut> {
    #[pin]
    writer: Option<Wr>,
    f: F,
    s: S,
    #[pin]
    future: Option<Fut>,
    _f: std::marker::PhantomData<fn(S)>,
}

impl<Wr, S, F, Fut> Bootstrapped<Wr, S, F, Fut> {
    pub(super) fn new(writer: Wr, s: S, f: F) -> Self {
        Self {
            writer: Some(writer),
            f,
            s,
            future: None,
            _f: std::marker::PhantomData,
        }
    }

    fn poll_new_writer<Part>(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Wr::Error>>
    where
        Wr: MultipartWrite<Part>,
        F: FnMut(&mut S) -> Fut,
        Fut: Future<Output = Result<Wr, Wr::Error>>,
    {
        let mut this = self.project();
        if this.future.is_none() {
            let fut = (this.f)(this.s);
            this.future.set(Some(fut));
        }
        let fut = this.future.as_mut().as_pin_mut().unwrap();
        match ready!(fut.poll(cx)) {
            Ok(wr) => {
                this.future.set(None);
                this.writer.set(Some(wr));
                Poll::Ready(Ok(()))
            }
            Err(e) => {
                this.future.set(None);
                Poll::Ready(Err(e))
            }
        }
    }
}

impl<Wr, S, F, Fut, Part> FusedMultipartWrite<Part> for Bootstrapped<Wr, S, F, Fut>
where
    Wr: FusedMultipartWrite<Part>,
    F: FnMut(&mut S) -> Fut,
    Fut: Future<Output = Result<Wr, Wr::Error>>,
{
    fn is_terminated(&self) -> bool {
        self.writer.is_none() && self.future.is_none()
    }
}

impl<Wr, S, F, Fut, Part> MultipartWrite<Part> for Bootstrapped<Wr, S, F, Fut>
where
    Wr: MultipartWrite<Part>,
    F: FnMut(&mut S) -> Fut,
    Fut: Future<Output = Result<Wr, Wr::Error>>,
{
    type Ret = Wr::Ret;
    type Output = Wr::Output;
    type Error = Wr::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.writer.is_none() {
            ready!(self.as_mut().poll_new_writer(cx))?;
        }
        let mut this = self.project();
        let wr = this.writer.as_mut().as_pin_mut().unwrap();
        wr.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, part: Part) -> Result<Self::Ret, Self::Error> {
        let mut this = self.project();
        assert!(this.writer.is_some());
        let wr = this.writer.as_mut().as_pin_mut().unwrap();
        wr.start_send(part)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let mut this = self.project();
        let wr = this
            .writer
            .as_mut()
            .as_pin_mut()
            .expect("polled Bootstrapped after completion");
        wr.poll_flush(cx)
    }

    fn poll_complete(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        let mut this = self.project();
        let wr = this
            .writer
            .as_mut()
            .as_pin_mut()
            .expect("polled Bootstrapped after completion");
        let output = ready!(wr.poll_complete(cx));
        this.writer.set(None);
        Poll::Ready(output)
    }
}

impl<Wr, S, F, Fut> Debug for Bootstrapped<Wr, S, F, Fut>
where
    Wr: Debug,
    S: Debug,
    Fut: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Bootstrapped")
            .field("writer", &self.writer)
            .field("s", &self.s)
            .field("future", &self.future)
            .finish()
    }
}
