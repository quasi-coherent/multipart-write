//! `MultipartWrite`rs compatible with [`Stream`].
use crate::MultipartWrite;

use futures::stream::Stream;

mod collect_complete;
pub use collect_complete::CollectComplete;

mod map_writer;
pub use map_writer::MapWriter;

/// Extension trait for combining streams with [`MultipartWrite`]rs.
pub trait MultipartStreamExt: Stream {
    /// Collects a stream by writing to a `MultipartWrite`, returning the
    /// result of completing the write as a future.
    fn collect_complete<W, Part>(self, writer: W) -> CollectComplete<Self, W, Part>
    where
        W: MultipartWrite<Part>,
        Self: Stream<Item = Result<Part, W::Error>> + Sized,
    {
        CollectComplete::new(self, writer)
    }

    /// Transforms this stream by writing its items into the given multipart
    /// writer and collecting the completed items as the transformed stream's
    /// item type.  The closure `F` determines when [`poll_complete`] is called.
    ///
    /// It is important that calling [`poll_complete`] leaves the writer in a
    /// consistent state to resume having parts written for a new object.
    fn map_writer<W, F>(self, writer: W, f: F) -> MapWriter<Self, W, F>
    where
        W: MultipartWrite<Self::Item>,
        F: FnMut(W::Ret) -> bool,
        Self: Sized,
    {
        MapWriter::new(self, writer, f)
    }
}

impl<St: Stream> MultipartStreamExt for St {}
