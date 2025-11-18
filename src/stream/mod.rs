//! Using `MultipartWrite` with streams.
//!
//! This module contains the extension [`MultipartStreamExt`] that has adapters
//! for composing `MultipartWrite` with streams.
use crate::{FusedMultipartWrite, MultipartWrite};

use futures_core::stream::Stream;

mod assemble;
pub use assemble::Assemble;

mod assembled;
pub use assembled::Assembled;

impl<St: Stream> MultipartStreamExt for St {}

/// An extension trait for `Stream`s that provides combinators to use with
/// `MultipartWrite`rs.
pub trait MultipartStreamExt: Stream {
    /// Collects a stream by writing to a `MultipartWrite`, returning the result
    /// of completing the write and assembling the parts in a future.
    fn assemble<Wr>(self, writer: Wr) -> Assemble<Self, Wr>
    where
        Wr: MultipartWrite<Self::Item>,
        Self: Sized,
    {
        Assemble::new(self, writer)
    }

    /// Writes the items of this stream to a `MultipartWrite`, completing the
    /// write when the closure returns true.
    ///
    /// The result is a stream of the results of polling the writer to
    /// completion.  If the inner writer becomes fused after producing an item,
    /// the stream is ended early.
    fn assembled<Wr, F>(self, writer: Wr, f: F) -> Assembled<Self, Wr, F>
    where
        Wr: FusedMultipartWrite<Self::Item>,
        F: FnMut(&Wr::Ret) -> bool,
        Self: Sized,
    {
        Assembled::new(self, writer, f)
    }
}
