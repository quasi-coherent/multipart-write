//! # Description
//!
//! This crate contains the trait `MultipartWrite`, assorted implementations,
//! and combinators.
//!
//! A `MultipartWrite` is a similar interface to [`Sink`], except that writing
//! an item or completing the write both return values.
//!
//! [Here][example] is a conceptual example of a `MultipartWrite`.
//!
//! For a more extensive use of the interface, see the crate
//! [`aws-multipart-upload`] where it is central to a "real world" and/or
//! "more serious" domain.
//!
//! # Motivation
//!
//! `Sink` is a useful API, but it is just that: a sink, the end of a stream.
//!
//! It's valuable to have the backpressure mechanism that `poll_ready` combined
//! with `start_send` enables, and it's nice to have the flexibility that the
//! shape of `Sink` provides in what kinds of values you can send with it.
//!
//! The idea for `MultipartWrite` is to:
//! 1. Allow the same desirable properies: backpressure and generic input type.
//! 2. Be able to be inserted earlier in a stream computation.
//! 3. Replace `Sink` when the use case would need a value returned by sending
//!    to it or closing it.
//! 4. Transform a stream by writing it in parts, which is somewhat of a
//!    specific rephrasing of the second and third points.
//!
//! [`Sink`]: https://docs.rs/crate/futures-sink/latest
//! [example]: https://github.com/quasi-coherent/multipart-write/blob/master/examples/author.rs
//! [`aws-multipart-upload`]: https://docs.rs/crate/aws-multipart-upload/latest
#![cfg_attr(docsrs, feature(doc_cfg))]
use std::collections::VecDeque;
use std::convert::Infallible as Never;
use std::ops::DerefMut;
use std::pin::Pin;
use std::task::{Context, Poll};

pub mod io;

pub mod stream;
#[doc(inline)]
pub use stream::MultipartStreamExt;

pub mod write;
#[doc(inline)]
pub use write::MultipartWriteExt;

/// `MultipartWrite` is an interface for asynchronously writing an object in
/// parts.
pub trait MultipartWrite<Part> {
    /// The type of value returned when sending a part to be written began
    /// successfully.
    type Recv;

    /// The type of value assembled from the parts when they have all been
    /// written.
    type Output;

    /// The type of value returned when an operation fails.
    type Error;

    /// Attempts to prepare the `MultipartWrite` to receive a new part.
    ///
    /// This method must be called and return `Poll::Ready` before each call to
    /// `start_send`, indicating that the underlying writer is ready to have
    /// another part written to it.
    ///
    /// This method returns `Poll::Pending` when the object being prepared
    /// cannot accept another part.
    ///
    /// # Errors
    ///
    /// Errors returned by this method are entirely implementation-specific but
    /// could render the writer permanently unusable.
    fn poll_ready(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>>;

    /// Begin the process of writing a part to this writer, returning the
    /// associated type confirming it was received successfully.
    ///
    /// This method must be preceded by a call to `poll_ready` that returns
    /// `Poll::Ready` to ensure that the `MultipartWrite` is ready to receive a
    /// new part.
    ///
    /// # Errors
    ///
    /// Errors returned by this method are entirely implementation-specific but
    /// could render the writer permanently unusable.  However, it is always an
    /// error to call `start_send` when `poll_ready` would return the value
    /// `Poll::Pending` indicating that the writer was not available to have a
    /// part written.
    fn start_send(
        self: Pin<&mut Self>,
        part: Part,
    ) -> Result<Self::Recv, Self::Error>;

    /// Flush any remaining output from the writer.
    ///
    /// Returns `Poll::Ready` when no unwritten parts remain and `Poll::Pending`
    /// if there is more work left to do.
    ///
    /// # Errors
    ///
    /// Errors returned by this method are entirely implementation-specific but
    /// could render the writer permanently unusable.
    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>>;

    /// Complete the write, returning the output assembled from the written
    /// parts.
    ///
    /// This method returns `Poll::Pending` until no buffered, unwritten parts
    /// remain and the complete output object is available.
    ///
    /// # Errors
    ///
    /// Errors returned by this method are entirely implementation-specific but
    /// could render the writer permanently unusable.
    fn poll_complete(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>>;
}

/// An owned, dynamically typed [`MultipartWrite`] for use in cases where it is
/// not possible or desirable to statically type it.
///
/// This is also handy to aid in type inference.  Since a `MultipartWrite` is
/// generic over the type of value being written, often it is not possible to
/// infer the types of `Part`, `R`, `T`, and/or `E`.
///
/// Erasing the type with `BoxMultipartWrite` picks the implementation and can
/// resolve a string of type inference failures.
pub type BoxMultipartWrite<'a, Part, R, T, E> = Pin<
    Box<dyn MultipartWrite<Part, Recv = R, Output = T, Error = E> + Send + 'a>,
>;

/// `BoxMultipartWrite` but without the `Send` requirement.
pub type LocalBoxMultipartWrite<'a, Part, R, T, E> =
    Pin<Box<dyn MultipartWrite<Part, Recv = R, Output = T, Error = E> + 'a>>;

/// A writer that tracks whether or not the underlying writer should no longer
/// be polled.
pub trait FusedMultipartWrite<Part>: MultipartWrite<Part> {
    /// Returns `true` if the writer should no longer be polled.
    fn is_terminated(&self) -> bool;
}

/// An owned, dynamically typed [`FusedMultipartWrite`] for use in cases where
/// it is not possible or desirable to statically type it.
///
/// This is also handy to aid in type inference.  Since a `FusedMultipartWrite`
/// is generic over the type of value being written, often it is not possible to
/// infer the types of `Part`, `R`, `T`, and/or `E`.
///
/// Erasing the type with `BoxFusedMultipartWrite` picks the implementation and
/// can resolve a string of type inference failures.
pub type BoxFusedMultipartWrite<'a, Part, R, T, E> = Pin<
    Box<
        dyn FusedMultipartWrite<Part, Recv = R, Output = T, Error = E>
            + Send
            + 'a,
    >,
>;

/// `BoxFusedMultipartWrite` but without the `Send` requirement.
pub type LocalBoxFusedMultipartWrite<'a, Part, R, T, E> = Pin<
    Box<dyn FusedMultipartWrite<Part, Recv = R, Output = T, Error = E> + 'a>,
>;

impl<W: ?Sized + MultipartWrite<Part> + Unpin, Part> MultipartWrite<Part>
    for &mut W
{
    type Error = W::Error;
    type Output = W::Output;
    type Recv = W::Recv;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut **self).poll_ready(cx)
    }

    fn start_send(
        mut self: Pin<&mut Self>,
        part: Part,
    ) -> Result<Self::Recv, Self::Error> {
        Pin::new(&mut **self).start_send(part)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut **self).poll_flush(cx)
    }

    fn poll_complete(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        Pin::new(&mut **self).poll_complete(cx)
    }
}

impl<W: ?Sized + FusedMultipartWrite<Part> + Unpin, Part>
    FusedMultipartWrite<Part> for &mut W
{
    fn is_terminated(&self) -> bool {
        <W as FusedMultipartWrite<Part>>::is_terminated(&**self)
    }
}

impl<W: ?Sized + MultipartWrite<Part> + Unpin, Part> MultipartWrite<Part>
    for Box<W>
{
    type Error = W::Error;
    type Output = W::Output;
    type Recv = W::Recv;

    fn poll_ready(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        Pin::new(self.get_mut().as_mut()).poll_ready(cx)
    }

    fn start_send(
        self: Pin<&mut Self>,
        part: Part,
    ) -> Result<Self::Recv, Self::Error> {
        Pin::new(self.get_mut().as_mut()).start_send(part)
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        Pin::new(self.get_mut().as_mut()).poll_flush(cx)
    }

    fn poll_complete(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        Pin::new(self.get_mut().as_mut()).poll_complete(cx)
    }
}

impl<W: ?Sized + FusedMultipartWrite<Part> + Unpin, Part>
    FusedMultipartWrite<Part> for Box<W>
{
    fn is_terminated(&self) -> bool {
        W::is_terminated(self)
    }
}

impl<P, Part> MultipartWrite<Part> for Pin<P>
where
    P: DerefMut + Unpin,
    P::Target: MultipartWrite<Part>,
{
    type Error = <P::Target as MultipartWrite<Part>>::Error;
    type Output = <P::Target as MultipartWrite<Part>>::Output;
    type Recv = <P::Target as MultipartWrite<Part>>::Recv;

    fn poll_ready(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.get_mut().as_mut().poll_ready(cx)
    }

    fn start_send(
        self: Pin<&mut Self>,
        part: Part,
    ) -> Result<Self::Recv, Self::Error> {
        self.get_mut().as_mut().start_send(part)
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
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

impl<T> MultipartWrite<T> for Vec<T> {
    type Error = Never;
    type Output = Self;
    type Recv = ();

    fn poll_ready(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(
        self: Pin<&mut Self>,
        part: T,
    ) -> Result<Self::Recv, Self::Error> {
        // SAFETY: We may treat `Vec<T>: Unpin` since we are not pinning the
        // elements.
        unsafe { self.get_unchecked_mut() }.push(part);
        Ok(())
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_complete(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        // SAFETY: We may treat `Vec<T>: Unpin` since we are not pinning the
        // elements.
        let this: &mut Vec<T> = unsafe { self.get_unchecked_mut() };
        let out = std::mem::take(this);
        Poll::Ready(Ok(out))
    }
}

impl<T> MultipartWrite<T> for VecDeque<T> {
    type Error = Never;
    type Output = Self;
    type Recv = ();

    fn poll_ready(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(
        self: Pin<&mut Self>,
        part: T,
    ) -> Result<Self::Recv, Self::Error> {
        // SAFETY: We may treat `VecDeque<T>: Unpin` since we are not pinning
        // the elements.
        unsafe { self.get_unchecked_mut() }.push_back(part);
        Ok(())
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_complete(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        // SAFETY: We may treat `VecDeque<T>: Unpin` since we are not pinning
        // the elements.
        let this: &mut VecDeque<T> = unsafe { self.get_unchecked_mut() };
        let out = std::mem::take(this);
        Poll::Ready(Ok(out))
    }
}
