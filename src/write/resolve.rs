use crate::{FusedMultipartWrite, MultipartWrite};

use futures_core::{Future, ready};
use std::fmt::{self, Debug, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

/// Create a `MultipartWrite` from a future that resolves to one.
pub fn resolve<Wr, E, Fut>(fut: Fut) -> Resolve<Fut, Wr>
where
    Fut: Future<Output = Result<Wr, E>>,
{
    Resolve::new(fut)
}

pin_project_lite::pin_project! {
    /// Writer returned by [`resolve`].
    #[must_use = "futures do nothing unless polled"]
    pub struct Resolve<Fut, Wr> {
        #[pin]
        fut: Option<Fut>,
        #[pin]
        writer: Option<Wr>,
    }
}

impl<Fut, Wr> Resolve<Fut, Wr> {
    fn new(fut: Fut) -> Self {
        Self {
            fut: Some(fut),
            writer: None,
        }
    }

    fn poll_writer<E>(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), E>>
    where
        Fut: Future<Output = Result<Wr, E>>,
    {
        if self.writer.is_some() {
            return Poll::Ready(Ok(()));
        }
        let mut this = self.project();
        let fut = this
            .fut
            .as_mut()
            .as_pin_mut()
            .expect("polled Resolve after completion");
        let writer = ready!(fut.poll(cx))?;
        this.writer.set(Some(writer));
        this.fut.set(None);

        Poll::Ready(Ok(()))
    }
}

impl<Wr, Part, E, Fut> FusedMultipartWrite<Part> for Resolve<Fut, Wr>
where
    Fut: Future<Output = Result<Wr, E>>,
    Wr: MultipartWrite<Part, Error = E>,
{
    fn is_terminated(&self) -> bool {
        self.fut.is_none() && self.writer.is_none()
    }
}

impl<Wr, Part, E, Fut> MultipartWrite<Part> for Resolve<Fut, Wr>
where
    Fut: Future<Output = Result<Wr, E>>,
    Wr: MultipartWrite<Part, Error = E>,
{
    type Ret = Wr::Ret;
    type Error = E;
    type Output = Wr::Output;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.as_mut().poll_writer(cx))?;

        let mut this = self.project();
        assert!(this.writer.is_some());

        this.writer.as_mut().as_pin_mut().unwrap().poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, part: Part) -> Result<Self::Ret, Self::Error> {
        let mut this = self.project();
        let writer = this
            .writer
            .as_mut()
            .as_pin_mut()
            .expect("polled Resolve after completion");
        writer.start_send(part)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let mut this = self.project();
        let writer = this
            .writer
            .as_mut()
            .as_pin_mut()
            .expect("polled Resolve after completion");
        writer.poll_flush(cx)
    }

    fn poll_complete(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        let mut this = self.project();
        let writer = this
            .writer
            .as_mut()
            .as_pin_mut()
            .expect("polled Resolve after completion");
        let out = ready!(writer.poll_complete(cx));
        this.writer.set(None);

        Poll::Ready(out)
    }
}

impl<Fut, Wr> Debug for Resolve<Fut, Wr>
where
    Wr: Debug,
    Fut: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Resolve")
            .field("fut", &self.fut)
            .field("writer", &self.writer)
            .finish()
    }
}
