//! Using `MultipartWrite` with streams.
//!
//! This module contains the extension [`MultipartStreamExt`] that has adapters
//! for composing `MultipartWrite` with streams.
use crate::MultipartWrite;

use futures_core::stream::Stream;

mod collect_writer;
pub use collect_writer::CollectWriter;

mod write_until;
pub use write_until::WriteUntil;

mod try_send;
pub use try_send::TrySend;

impl<St: Stream> MultipartStreamExt for St {}

/// An extension trait for `Stream`s that provides combinators to use with
/// `MultipartWrite`rs.
pub trait MultipartStreamExt: Stream {
    /// Collects a stream by writing to a `MultipartWrite`, returning the
    /// result of completing the write as a future.
    fn collect_writer<Wr>(self, writer: Wr) -> CollectWriter<Self, Wr>
    where
        Wr: MultipartWrite<Self::Item>,
        Self: Sized,
    {
        CollectWriter::new(self, writer)
    }

    /// Call `start_send` on the provided writer with the items of this stream,
    /// producing a stream of results in the return value `Wr::Ret`.
    fn try_send<Wr>(self, writer: Wr) -> TrySend<Self, Wr>
    where
        Wr: MultipartWrite<Self::Item>,
        Self: Sized,
    {
        TrySend::new(self, writer)
    }

    /// Sends items of this stream to the writer, yielding the completed written
    /// value as the next item in the resulting stream when the provided closure
    /// returns `true`.
    fn write_until<Wr, F>(self, writer: Wr, f: F) -> WriteUntil<Self, Wr, F>
    where
        Wr: MultipartWrite<Self::Item>,
        F: FnMut(Wr::Ret) -> bool,
        Self: Sized,
    {
        WriteUntil::new(self, writer, f)
    }
}
