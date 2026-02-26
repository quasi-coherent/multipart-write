use std::fmt::{self, Debug, Formatter};
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::{Future, ready};

use crate::{FusedMultipartWrite, MultipartWrite};

/// Create a `MultipartWrite` from a future that resolves to one.
///
/// Call `poll_ready` first to satisfy the API contract.  The first time
/// `poll_ready` is called creates the writer and returns `Poll::Pending` until
/// it exists. inner writer type, at which point it can be used normally.
///
/// A `poll_complete` does not drop the inner writer, but it  may or may not be
/// usable.  It depends on the particular implementation; `Resolve` defers to
/// the inner writer for fusing behavior.
///
/// # Panics
///
/// Panics when `poll_ready` is not called first in order to poll the provided
/// future resolving to the writer.
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
        Self { fut: Some(fut), writer: None }
    }

    /// Returns whether this writer has had the inner writer created.
    pub fn has_writer(&self) -> bool {
        self.writer.is_some()
    }

    /// Consumes `Resolve`, returning the underlying writer.
    pub fn into_inner(self) -> Option<Wr> {
        self.writer
    }

    /// Acquires a reference to the underlying writer.
    pub fn as_ref(&self) -> Option<&Wr> {
        self.writer.as_ref()
    }

    /// Returns a reference to the inner writer's `Deref::Target`.
    pub fn as_deref(&self) -> Option<&Wr::Target>
    where
        Wr: Deref,
    {
        self.writer.as_deref()
    }

    /// Returns a mutable reference to the inner writer's `Deref::Target`.
    pub fn as_deref_mut(&mut self) -> Option<&mut Wr::Target>
    where
        Wr: DerefMut,
    {
        self.writer.as_deref_mut()
    }

    /// Project a pin onto the inner writer, returning it in an option.
    pub fn as_pin_mut(self: Pin<&mut Self>) -> Option<Pin<&mut Wr>> {
        self.project().writer.as_pin_mut()
    }

    fn poll_writer<E>(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), E>>
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
            .expect("missing both writer and future to create it");
        let writer = ready!(fut.poll(cx))?;
        this.writer.set(Some(writer));
        this.fut.set(None);

        Poll::Ready(Ok(()))
    }
}

impl<Wr, Part, E, Fut> FusedMultipartWrite<Part> for Resolve<Fut, Wr>
where
    Fut: Future<Output = Result<Wr, E>>,
    Wr: FusedMultipartWrite<Part, Error = E>,
{
    fn is_terminated(&self) -> bool {
        self.fut.is_none()
            && self.writer.as_ref().is_some_and(|w| w.is_terminated())
    }
}

impl<Wr, Part, E, Fut> MultipartWrite<Part> for Resolve<Fut, Wr>
where
    Fut: Future<Output = Result<Wr, E>>,
    Wr: MultipartWrite<Part, Error = E>,
{
    type Error = E;
    type Output = Wr::Output;
    type Recv = Wr::Recv;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        ready!(self.as_mut().poll_writer(cx))?;
        let mut this = self.project();

        this.writer
            .as_mut()
            .as_pin_mut()
            .expect("inner writer lost unexpectedly")
            .poll_ready(cx)
    }

    fn start_send(
        self: Pin<&mut Self>,
        part: Part,
    ) -> Result<Self::Recv, Self::Error> {
        let mut this = self.project();
        let writer = this
            .writer
            .as_mut()
            .as_pin_mut()
            .expect("start_send with no existing writer");
        writer.start_send(part)
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        let mut this = self.project();
        let writer = this
            .writer
            .as_mut()
            .as_pin_mut()
            .expect("poll_flush with no existing writer");
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
            .expect("poll_complete with no existing writer");
        let out = ready!(writer.poll_complete(cx));

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
