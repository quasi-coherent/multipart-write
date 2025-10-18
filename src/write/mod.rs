//! `MultipartWrite` combinators.
//!
//! This module contains the trait [`MultipartWriteExt`], which provides adapters
//! for chaining and composing [`MultipartWrite`]rs.
use crate::MultipartWrite;

use futures::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

mod buffered;
pub use buffered::Buffered;

mod complete;
pub use complete::Complete;

mod feed;
pub use feed::Feed;

mod fold_ret;
pub use fold_ret::FoldRet;

mod flush;
pub use flush::Flush;

mod map;
pub use map::Map;

mod map_err;
pub use map_err::MapErr;

mod map_ret;
pub use map_ret::MapRet;

mod send;
pub use send::Send;

mod then;
pub use then::Then;

mod with;
pub use with::With;

/// An extension trait for `MultipartWrite`rs providing a variety of convenient
/// combinator functions.
pub trait MultipartWriteExt<Part>: MultipartWrite<Part> {
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

    /// A future that runs this writer to completion, returning the associated
    /// output.
    fn complete(&mut self) -> Complete<'_, Self, Part>
    where
        Self: Unpin,
    {
        Complete::new(self)
    }

    /// A future that completes after the given part has been received by the
    /// writer.
    ///
    /// Unlike `write`, the returned future does not flush the writer.  It is the
    /// caller's responsibility  to ensure all pending items are processed, which
    /// can be done with `flush` or `complete`.
    fn feed(&mut self, part: Part) -> Feed<'_, Self, Part>
    where
        Self: Unpin,
    {
        Feed::new(self, part)
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
    fn fold_ret<T, F>(self, id: T, f: F) -> FoldRet<Self, F, T>
    where
        F: FnMut(T, &Self::Ret) -> T,
        Self: Sized,
    {
        FoldRet::new(self, id, f)
    }

    /// Map this writer's output type to a different type, returning a new
    /// multipart writer with the given output type.
    fn map<U, F>(self, f: F) -> Map<Self, F>
    where
        F: FnMut(Self::Output) -> U,
        Self: Sized,
    {
        Map::new(self, f)
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
    fn map_ret<U, F>(self, f: F) -> MapRet<Self, F>
    where
        F: FnMut(Self::Ret) -> U,
        Self: Sized,
    {
        MapRet::new(self, f)
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

    /// A convenience method for calling [`poll_complete`] on [`Unpin`] writer types.
    ///
    /// [`poll_complete`]: super::MultipartWrite::poll_complete
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
    fn send(&mut self, part: Part) -> Send<'_, Self, Part>
    where
        Self: Unpin,
    {
        Send::new(self, part)
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
    fn with<U, Fut, F, E>(self, f: F) -> With<Self, Part, U, Fut, F>
    where
        F: FnMut(U) -> Fut,
        Fut: Future<Output = Result<Part, E>>,
        E: From<Self::Error>,
        Self: Sized,
    {
        With::new(self, f)
    }
}

impl<W: MultipartWrite<Part>, Part> MultipartWriteExt<Part> for W {}
