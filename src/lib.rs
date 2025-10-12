//! # multipart-write
//!
//! This crate contains the trait [MultipartWrite] and assorted implementations
//! and convenience combinators.
//!
//! A [MultipartWrite] is a similar interface as [Sink], except that it allows
//! returning a value when writing a part or completing the overall write.
//!
//! [Sink]: futures::sink::Sink
use std::ops::DerefMut;
use std::pin::Pin;
use std::task::{Context, Poll};

pub mod write;

pub mod prelude {
    pub use super::write::MultipartWriteExt as _;
    pub use super::write::MultipartWriteStreamExt as _;
}

/// `MultipartWrite` is a [Sink]-like interface for asynchronously writing an
/// object in parts.
///
/// [Sink]: futures::sink::Sink
pub trait MultipartWrite<Part> {
    /// The type of value returned when writing the part began successfully.
    type Ret;

    /// The type of value returned when all parts are written.
    type Output;

    /// The type of value returned when an operation fails.
    type Error;

    /// Attempts to prepare the `MultipartWrite` to receive a new part.
    ///
    /// This method must be called and return `Poll::Ready` before each call to
    /// `start_write`, indicating that the underlying writer is ready to have
    /// another part written to it.
    ///
    /// This method returns `Poll::Pending` when the object being prepared cannot
    /// accept another part.  In this case, `poll_freeze` should be called to
    /// finish the multipart write of this object in order to begin writing a new
    /// one.
    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>;

    /// Begin the process of writing a part to this writer, returning the
    /// associated type confirming this was done successfully.
    ///
    /// Like [Sink], this should be preceded by a call to `poll_ready` that
    /// returns `Poll::Ready` to ensure that the `MultipartWrite` is ready to
    /// receive a new part.
    ///
    /// # Errors
    ///
    /// Errors returned by this method are implementation-specific, but it is
    /// always an error to call `start_write` when the writer is not available.
    ///
    /// [Sink]: futures::sink::Sink
    fn start_write(self: Pin<&mut Self>, part: Part) -> Result<Self::Ret, Self::Error>;

    /// Flush any remaining output from the writer.
    ///
    /// Returns `Poll::Ready` when no parts remain and `Poll::Pending` if there
    /// is more work left to do.
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>;

    /// Freeze this writer, returning the output.
    ///
    /// This method returns `Poll::Ready` when no buffered, unwritten parts
    /// remain and the complete output object is available.
    fn poll_freeze(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>>;
}

impl<W: ?Sized + MultipartWrite<Part> + Unpin, Part> MultipartWrite<Part> for &mut W {
    type Ret = W::Ret;
    type Output = W::Output;
    type Error = W::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut **self).poll_ready(cx)
    }

    fn start_write(mut self: Pin<&mut Self>, part: Part) -> Result<Self::Ret, Self::Error> {
        Pin::new(&mut **self).start_write(part)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut **self).poll_flush(cx)
    }

    fn poll_freeze(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        Pin::new(&mut **self).poll_freeze(cx)
    }
}

impl<P, Part> MultipartWrite<Part> for Pin<P>
where
    P: DerefMut + Unpin,
    P::Target: MultipartWrite<Part>,
{
    type Ret = <P::Target as MultipartWrite<Part>>::Ret;
    type Output = <P::Target as MultipartWrite<Part>>::Output;
    type Error = <P::Target as MultipartWrite<Part>>::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.get_mut().as_mut().poll_ready(cx)
    }

    fn start_write(self: Pin<&mut Self>, part: Part) -> Result<Self::Ret, Self::Error> {
        self.get_mut().as_mut().start_write(part)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.get_mut().as_mut().poll_flush(cx)
    }

    fn poll_freeze(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        self.get_mut().as_mut().poll_freeze(cx)
    }
}
