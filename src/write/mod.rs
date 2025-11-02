//! `MultipartWrite` combinators.
//!
//! This module contains the trait [`MultipartWriteExt`], which provides adapters
//! for chaining and composing `MultipartWrite`rs.
use futures_core::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

pub use crate::{BoxMultipartWrite, LocalBoxMultipartWrite, MultipartWrite};

mod and_then;
pub use and_then::AndThen;

mod buffered;
pub use buffered::Buffered;

mod clear_with;
pub use clear_with::ClearWith;

mod complete;
pub use complete::Complete;

mod fanout;
pub use fanout::Fanout;

mod feed;
pub use feed::Feed;

mod filter;
pub use filter::Filter;

mod filter_map;
pub use filter_map::FilterMap;

mod fold_ret;
pub use fold_ret::FoldRet;

mod flush;
pub use flush::Flush;

mod map_err;
pub use map_err::MapErr;

mod map_ok;
pub use map_ok::MapOk;

mod returning;
pub use returning::Returning;

mod send_part;
pub use send_part::SendPart;

mod with;
pub use with::With;

impl<Wr: MultipartWrite<Part>, Part> MultipartWriteExt<Part> for Wr {}

/// An extension trait for `MultipartWrite` providing a variety of convenient
/// combinator functions.
pub trait MultipartWriteExt<Part>: MultipartWrite<Part> {
    /// Compute from this writer's output type a new output of a different type
    /// using an asynchronous closure.
    ///
    /// Calling `poll_complete` on this writer will complete the inner writer,
    /// then run the provided closure `f` with the output to produce the final
    /// output of this writer.
    fn and_then<T, E, Fut, F>(self, f: F) -> AndThen<Self, Fut, F>
    where
        F: FnMut(Self::Output) -> Fut,
        Fut: Future<Output = Result<T, E>>,
        E: From<Self::Error>,
        Self: Sized,
    {
        AndThen::new(self, f)
    }

    /// Wrap this writer in a `Box`, pinning it.
    fn boxed<'a>(self) -> BoxMultipartWrite<'a, Part, Self::Ret, Self::Output, Self::Error>
    where
        Self: Sized + Send + 'a,
    {
        Box::pin(self)
    }

    /// Wrap this writer in a `Box`, pinning it.
    ///
    /// Similar to `boxed` but without the `Send` requirement.
    fn boxed_local<'a>(
        self,
    ) -> LocalBoxMultipartWrite<'a, Part, Self::Ret, Self::Output, Self::Error>
    where
        Self: Sized + 'a,
    {
        Box::pin(self)
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

    /// Computes the closure before returning the completed output.
    ///
    /// This can be used to reset some internal state of the writer in order to
    /// prepare it to be written to again.
    fn clear_with<Wr, T, F>(self, val: T, f: F) -> ClearWith<Self, T, F>
    where
        F: FnMut(&mut Self, &T),
        Self: Sized + Unpin,
    {
        ClearWith::new(self, val, f)
    }

    /// A future that runs this writer to completion, returning the associated
    /// output.
    fn complete(&mut self) -> Complete<'_, Self, Part>
    where
        Self: Unpin,
    {
        Complete::new(self)
    }

    /// Fanout the part to multiple writers.
    ///
    /// This adapter clones each incoming part and forwards it to both writers.
    fn fanout<U>(self, other: U) -> Fanout<Self, U, Part>
    where
        Part: Clone,
        U: MultipartWrite<Part, Error = Self::Error>,
        Self: Sized,
    {
        Fanout::new(self, other)
    }

    /// A future that completes after the given part has been received by the
    /// writer.
    ///
    /// Unlike `write`, the returned future does not flush the writer.  It is the
    /// caller's responsibility to ensure all pending items are processed, which
    /// can be done with `flush` or `complete`.
    fn feed(&mut self, part: Part) -> Feed<'_, Self, Part>
    where
        Self: Unpin,
    {
        Feed::new(self, part)
    }

    /// Apply a filter to this writer's parts, returning a new writer with the
    /// same output.
    ///
    /// The return type of this writer is `Option<Self::Ret>` and is `None` when
    /// the part did not pass the filter.
    fn filter<F>(self, f: F) -> Filter<Self, F>
    where
        F: FnMut(&Part) -> bool,
        Self: Sized,
    {
        Filter::new(self, f)
    }

    /// Attempt to map the input to a part for this writer, filtering out the
    /// inputs where the mapping returns `None`.
    ///
    /// The return type of this writer is `Option<Self::Ret>` and is `None` when
    /// the mapping of the input `U` did not pass the filter.
    fn filter_map<U, F>(self, f: F) -> FilterMap<Self, F>
    where
        F: FnMut(U) -> Option<Part>,
        Self: Sized,
    {
        FilterMap::new(self, f)
    }

    /// A future that completes when the underlying writer has been flushed.
    fn flush(&mut self) -> Flush<'_, Self, Part>
    where
        Self: Unpin,
    {
        Flush::new(self)
    }

    /// Accumulate this writer's returned values, returning a new multipart
    /// writer that pairs the underlying writer's output with the
    /// result of the accumulating function.
    fn fold_ret<T, F>(self, id: T, f: F) -> FoldRet<Self, T, F>
    where
        F: FnMut(T, &Self::Ret) -> T,
        Self: Sized,
    {
        FoldRet::new(self, id, f)
    }

    /// Map this writer's output type to a different type, returning a new
    /// multipart writer with the given output type.
    fn map_ok<U, F>(self, f: F) -> MapOk<Self, F>
    where
        F: FnMut(Self::Output) -> U,
        Self: Sized,
    {
        MapOk::new(self, f)
    }

    /// Map this writer's error type to a different value, returning a new
    /// multipart writer with the given error type.
    fn map_err<E, F>(self, f: F) -> MapErr<Self, F>
    where
        F: FnMut(Self::Error) -> E,
        Self: Sized,
    {
        MapErr::new(self, f)
    }

    /// Map this writer's return type to a different value, returning a new
    /// multipart writer with the given return type.
    fn returning<U, F>(self, f: F) -> Returning<Self, F>
    where
        F: FnMut(Self::Ret) -> U,
        Self: Sized,
    {
        Returning::new(self, f)
    }

    /// A convenience method for calling [`MultipartWrite::poll_ready`] on
    /// [`Unpin`] writer types.
    #[must_use = "futures do nothing unless polled"]
    fn poll_ready_unpin(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>
    where
        Self: Unpin,
    {
        Pin::new(self).poll_ready(cx)
    }

    /// A convenience method for calling [`MultipartWrite::poll_flush`] on
    /// [`Unpin`] writer types.
    #[must_use = "futures do nothing unless polled"]
    fn poll_flush_unpin(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>
    where
        Self: Unpin,
    {
        Pin::new(self).poll_flush(cx)
    }

    /// A convenience method for calling [`MultipartWrite::poll_complete`] on
    /// [`Unpin`] writer types.
    #[must_use = "futures do nothing unless polled"]
    fn poll_complete_unpin(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>>
    where
        Self: Unpin,
    {
        Pin::new(self).poll_complete(cx)
    }

    /// A future that completes when a part has been fully processed into the
    /// writer, including flushing.
    fn send_part(&mut self, part: Part) -> SendPart<'_, Self, Part>
    where
        Self: Unpin,
    {
        SendPart::new(self, part)
    }

    /// Provide a part to this writer in the output of a future.
    ///
    /// The result is a new writer over the type `U` that passes each value
    /// through the function `f`, resolving the output, and sending it to the
    /// inner writer.
    fn with<U, E, Fut, F>(self, f: F) -> With<Self, Part, Fut, F>
    where
        F: FnMut(U) -> Fut,
        Fut: Future<Output = Result<Part, E>>,
        E: From<Self::Error>,
        Self: Sized,
    {
        With::new(self, f)
    }
}
