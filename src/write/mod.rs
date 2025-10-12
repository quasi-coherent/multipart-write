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

mod then;
pub use then::Then;

mod frozen;
pub use frozen::Frozen;

mod with;
pub use with::With;

mod write_part;
pub use write_part::WritePart;

/// An extension trait for `MultipartWrite`rs that provides a variety of
/// convenient combinator functions.
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

    /// Buffer the output of this writer, returning writer outputs in the order
    /// they complete.  This writer returns `Some(output)` until there is no more
    /// buffered output.
    ///
    /// The capacity imposes a limit on the number of concurrent writer's
    /// completing the output.  A limit of `Some(0)` or `None` both correspond to
    /// an unbounded buffer.
    fn buffered(self, capacity: impl Into<Option<usize>>) -> Buffered<Self, Part>
    where
        Self: Sized,
    {
        Buffered::new(self, capacity.into().unwrap_or_default())
    }

    /// Write a part to this writer.
    fn write_part(&mut self, part: Part) -> WritePart<'_, Self, Part>
    where
        Self: Unpin,
    {
        WritePart::new(self, part)
    }

    /// Flushes this writer, ensuring all parts have been written.
    fn flush(&mut self) -> Flush<'_, Self, Part>
    where
        Self: Unpin,
    {
        Flush::new(self)
    }

    /// Runs this writer to completion, returning the associated output.
    fn freeze(&mut self) -> Freeze<'_, Self, Part>
    where
        Self: Unpin,
    {
        Freeze::new(self)
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

pub trait MultipartWriteStreamExt<Part>: Stream<Item = Part> {
    /// Map this stream into a `MultipartWrite`, returning a new stream whose
    /// item type is the `MultiPartWrite`r's `Output` type.
    ///
    /// Provide the closure with the logic to flush/freeze the writer given the
    /// value returned by writing one part.
    fn frozen<W, F>(self, writer: W, f: F) -> Frozen<Self, W, F>
    where
        W: MultipartWrite<Part>,
        F: FnMut(W::Ret) -> bool,
        Self: Sized,
    {
        Frozen::new(self, writer, f)
    }
}
