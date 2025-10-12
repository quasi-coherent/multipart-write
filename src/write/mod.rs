//! `MultipartWrite` combinators.
//!
//! This module contains the traits [`MultipartWriteExt`] and
//! [`MultipartWriteStreamExt`], which provide adapters for chaining and
//! composing [`MultipartWrite`] types, or combining them with common [`futures`]
//! interfaces.
use crate::MultipartWrite;

use futures::future::Future;
use futures::stream::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};

mod buffered;
pub use buffered::Buffered;

mod flush;
pub use flush::Flush;

mod freeze;
pub use freeze::Freeze;

mod map;
pub use map::Map;

mod map_err;
pub use map_err::MapErr;

mod multipart_writer_sink;
pub use multipart_writer_sink::MultipartWriterSink;

mod then;
pub use then::Then;

mod frozen;
pub use frozen::Frozen;

mod with;
pub use with::With;

mod write_part;
pub use write_part::WritePart;

/// An extension trait for `MultipartWrite`rs providing a variety of convenient
/// combinator functions.
pub trait MultipartWriteExt<Part>: MultipartWrite<Part> {
    /// Map this writer's output type to a different type, resulting in a new
    /// multipart writer of the resulting type.
    fn map<U, F>(self, f: F) -> Map<Self, F>
    where
        F: FnMut(Self::Output) -> U,
        Self: Sized,
    {
        Map::new(self, f)
    }

    /// Map this writer's error type to a different value.
    fn map_err<E, F>(self, f: F) -> MapErr<Self, F>
    where
        F: FnOnce(Self::Error) -> E,
        Self: Sized,
    {
        MapErr::new(self, f)
    }

    /// Chain a computation on the output of a writer.
    fn then<U, F, Fut>(self, f: F) -> Then<Self, F, Fut>
    where
        F: FnMut(Self::Output) -> Fut,
        Fut: Future<Output = U>,
        Self: Sized,
    {
        Then::new(self, f)
    }

    /// Composes a function in front of the `MultipartWrite`.
    ///
    /// This adapter produces a new `MultipartWrite` by passing each part through
    /// the given function `f` before sending it to `self`.
    fn with<Q, Fut, F, E>(self, f: F) -> With<Self, Q, Part, F, Fut>
    where
        F: FnMut(Q) -> Fut,
        Fut: Future<Output = Result<Part, E>>,
        E: From<Self::Error>,
        Self: Sized,
    {
        With::new(self, f)
    }

    /// Adds a fixed size buffer to the current writer.
    ///
    /// The resulting `MultipartWrite` will buffer up to `capacity` items when
    /// the underlying writer is not able to accept new parts.
    fn buffered(self, capacity: impl Into<Option<usize>>) -> Buffered<Self, Part>
    where
        Self: Sized,
    {
        Buffered::new(self, capacity.into().unwrap_or_default())
    }

    /// Convert this writer into a [`Sink`].
    ///
    /// [`Sink`]: futures::sink::Sink
    fn into_sink(self) -> MultipartWriterSink<Self>
    where
        Self: Sized,
    {
        MultipartWriterSink::new(self)
    }

    /// A future that completes when a part has been written to the writer.
    fn write_part(&mut self, part: Part) -> WritePart<'_, Self, Part>
    where
        Self: Unpin,
    {
        WritePart::new(self, part)
    }

    /// A future that completes when the underlying writer has been flushed.
    fn flush(&mut self) -> Flush<'_, Self, Part>
    where
        Self: Unpin,
    {
        Flush::new(self)
    }

    /// A future that runs this writer to completion, returning the associated
    /// output.
    fn freeze(&mut self) -> Freeze<'_, Self, Part>
    where
        Self: Unpin,
    {
        Freeze::new(self)
    }

    /// A convenience method for calling [`poll_ready`] on [`Unpin`] writer types.
    ///
    /// [`poll_ready`]: super::MultipartWrite::poll_ready
    fn poll_ready_unpin(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>
    where
        Self: Unpin,
    {
        Pin::new(self).poll_ready(cx)
    }

    /// A convenience method for calling [`poll_flush`] on [`Unpin`] writer types.
    ///
    /// [`poll_flush`]: super::MultipartWrite::poll_flush
    fn poll_flush_unpin(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>
    where
        Self: Unpin,
    {
        Pin::new(self).poll_flush(cx)
    }

    /// A convenience method for calling [`poll_freeze`] on [`Unpin`] writer types.
    ///
    /// [`poll_freeze`]: super::MultipartWrite::poll_freeze
    fn poll_freeze_unpin(&mut self, cx: &mut Context<'_>) -> Poll<Result<Self::Output, Self::Error>>
    where
        Self: Unpin,
    {
        Pin::new(self).poll_freeze(cx)
    }
}

impl<W: MultipartWrite<Part>, Part> MultipartWriteExt<Part> for W {}

/// A [`Stream`] extension for combining streams with [`MultipartWrite`]rs.
///
/// [`Stream`]: futures::stream::Stream
/// [`MultipartWrite`]: crate::MultipartWrite
pub trait MultipartWriteStreamExt<Part>: Stream<Item = Part> {
    /// Map this stream into a `MultipartWrite`, returning a new stream whose
    /// item type is the `MultiPartWrite`r's `Output` type.
    ///
    /// The function of the closure is to determine from the associated
    /// [`MultipartWrite::Ret`] when it is appropriate to flush/freeze the writer
    /// and produce the output as the next item in the stream.
    fn frozen<W, F>(self, writer: W, f: F) -> Frozen<Self, W, F>
    where
        W: MultipartWrite<Part>,
        F: FnMut(W::Ret) -> bool,
        Self: Sized,
    {
        Frozen::new(self, writer, f)
    }
}

impl<St: Stream<Item = Part>, Part> MultipartWriteStreamExt<Part> for St {}
