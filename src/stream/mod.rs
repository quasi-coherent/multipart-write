//! Using `MultipartWrite` with streams.
//!
//! This module contains the extension [`MultipartStreamExt`] that has adapters
//! for composing `MultipartWrite` with streams.
use crate::MultipartWrite;

use futures_core::stream::Stream;

mod collect_complete;
pub use collect_complete::CollectComplete;

mod complete_when;
pub use complete_when::CompleteWhen;

mod try_send;
pub use try_send::TrySend;

impl<St: Stream> MultipartStreamExt for St {}

/// An extension trait for `Stream`s that provides combinators to use with
/// `MultipartWrite`rs.
pub trait MultipartStreamExt: Stream {
    /// Collects a stream by writing to a `MultipartWrite`, returning the
    /// result of completing the write as a future.
    fn collect_complete<Wr>(self, writer: Wr) -> CollectComplete<Self, Wr>
    where
        Wr: MultipartWrite<Self::Item>,
        Self: Sized,
    {
        CollectComplete::new(self, writer)
    }

    /// Sends items of this stream to the writer, yielding the completed written
    /// value as the next item in the resulting stream when the provided closure
    /// returns `true`.
    fn complete_when<Wr, F>(self, writer: Wr, f: F) -> CompleteWhen<Self, Wr, F>
    where
        Wr: MultipartWrite<Self::Item>,
        F: FnMut(Wr::Ret) -> bool,
        Self: Sized,
    {
        CompleteWhen::new(self, writer, f)
    }

    /// Call `start_send` on the provided writer with the items of this stream,
    /// yielding the output of the call as items for the resulting stream.
    fn try_send<Wr>(self, writer: Wr) -> TrySend<Self, Wr>
    where
        Wr: MultipartWrite<Self::Item>,
        Self: Sized,
    {
        TrySend::new(self, writer)
    }
}
