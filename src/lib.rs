//! # Description
//!
//! This crate contains the trait `MultipartWrite`, assorted implementations,
//! and combinators.
//!
//! A `MultipartWrite` is a similar interface to [`Sink`], except that writing
//! an item or completing the write both return values.
//!
//! See a conceptual example of a `MultipartWrite` [here][example].
//!
//! # Motivation
//!
//! `Sink` is a useful API, but it is just that--a sink.  The end of a stream.
//! It's useful to have the backpressure mechanism that `poll_ready`/`start_send`
//! enables while being able to act as a stream itself and produce items that
//! can be forwarded for more processing.
//!
//! [example]: https://github.com/quasi-coherent/multipart-write/blob/2cfd8bab323132ba3c0caa9f31b33b45d9faf8c1/examples/author.rs
//! [`Sink`]: https://docs.rs/crate/futures-sink/0.3.31
#![cfg_attr(docsrs, feature(doc_cfg))]
use std::ops::DerefMut;
use std::pin::Pin;
use std::task::{Context, Poll};

pub mod io;
pub mod stream;
pub mod write;

/// A prelude for this crate.
pub mod prelude {
    pub use super::stream::{self, MultipartStreamExt as _};
    pub use super::write::{self, MultipartWriteExt as _};
    pub use super::{FusedMultipartWrite, MultipartWrite};
}

/// `MultipartWrite` is a `Sink`-like interface for asynchronously writing an
/// object in parts.
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
    /// `start_send`, indicating that the underlying writer is ready to have
    /// another part written to it.
    ///
    /// This method returns `Poll::Pending` when the object being prepared cannot
    /// accept another part.
    ///
    /// In most cases, if the writer encounters an error, it will be permanently
    /// unable to write more parts.
    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>;

    /// Begin the process of writing a part to this writer, returning the
    /// associated type confirming this was done successfully.
    ///
    /// Like `Sink`, this should be preceded by a call to `poll_ready` that
    /// returns `Poll::Ready` to ensure that the `MultipartWrite` is ready to
    /// receive a new part.
    ///
    /// # Errors
    ///
    /// Errors returned by this method are implementation-specific, but it is
    /// always an error to call `start_send` when `poll_ready` would return
    /// `Poll::Pending`.
    ///
    /// In most cases, if the writer encounters an error, it will be permanently
    /// unable to write more parts.
    fn start_send(self: Pin<&mut Self>, part: Part) -> Result<Self::Ret, Self::Error>;

    /// Flush any remaining output from the writer.
    ///
    /// Returns `Poll::Ready` when no buffered, unwritten parts remain and
    /// `Poll::Pending` if there is more work left to do.
    ///
    /// In most cases, if the writer encounters an error, it will be permanently
    /// unable to write more parts.
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>;

    /// Complete this writer, returning the output.
    ///
    /// This method returns `Poll::Pending` until no buffered, unwritten parts
    /// remain and the complete output object is available.
    ///
    /// In most cases, if the writer encounters an error, it will be permanently
    /// unable to write more parts.
    fn poll_complete(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>>;
}

/// An owned, dynamically typed [`MultipartWrite`] for use in cases where it is
/// not possible or desirable to statically type it.
pub type BoxMultipartWrite<'a, Part, R, T, E> =
    Pin<Box<dyn MultipartWrite<Part, Ret = R, Output = T, Error = E> + Send + 'a>>;

/// `BoxMultipartWrite` but without the `Send` requirement.
pub type LocalBoxMultipartWrite<'a, Part, R, T, E> =
    Pin<Box<dyn MultipartWrite<Part, Ret = R, Output = T, Error = E> + 'a>>;

/// A writer that tracks whether or not the underlying writer should no longer
/// be polled.
pub trait FusedMultipartWrite<Part>: MultipartWrite<Part> {
    /// Returns `true` if the writer should no longer be polled.
    fn is_terminated(&self) -> bool;
}

impl<W: ?Sized + MultipartWrite<Part> + Unpin, Part> MultipartWrite<Part> for &mut W {
    type Ret = W::Ret;
    type Output = W::Output;
    type Error = W::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut **self).poll_ready(cx)
    }

    fn start_send(mut self: Pin<&mut Self>, part: Part) -> Result<Self::Ret, Self::Error> {
        Pin::new(&mut **self).start_send(part)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut **self).poll_flush(cx)
    }

    fn poll_complete(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        Pin::new(&mut **self).poll_complete(cx)
    }
}

impl<W: ?Sized + FusedMultipartWrite<Part> + Unpin, Part> FusedMultipartWrite<Part> for &mut W {
    fn is_terminated(&self) -> bool {
        <W as FusedMultipartWrite<Part>>::is_terminated(&**self)
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

    fn start_send(self: Pin<&mut Self>, part: Part) -> Result<Self::Ret, Self::Error> {
        self.get_mut().as_mut().start_send(part)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.get_mut().as_mut().poll_flush(cx)
    }

    fn poll_complete(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        self.get_mut().as_mut().poll_complete(cx)
    }
}

impl<P, Part> FusedMultipartWrite<Part> for Pin<P>
where
    P: DerefMut + Unpin,
    P::Target: FusedMultipartWrite<Part>,
{
    fn is_terminated(&self) -> bool {
        <P::Target as FusedMultipartWrite<Part>>::is_terminated(&**self)
    }
}
