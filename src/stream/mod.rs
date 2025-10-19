//! `MultipartWrite`rs compatible with [`Stream`].
use crate::MultipartWrite;

use futures::stream::Stream;

mod collect_completed;
pub use collect_completed::CollectCompleted;

mod feed_multipart_write;
pub use feed_multipart_write::FeedMultipartWrite;

impl<St: Stream> MultipartStreamExt for St {}

/// Extension trait for combining streams with [`MultipartWrite`]rs.
pub trait MultipartStreamExt: Stream {
    /// This adapter transforms the stream into a new stream whose item type is
    /// the output of the multipart writer writing parts until the closure `F`
    /// indicates the writer should be completed.
    fn feed_multipart_write<Wr, F>(self, writer: Wr, f: F) -> FeedMultipartWrite<Self, Wr, F>
    where
        Wr: MultipartWrite<Self::Item>,
        F: FnMut(Wr::Ret) -> bool,
        Self: Sized,
    {
        FeedMultipartWrite::new(self, writer, f)
    }

    /// Collects a stream by writing to a `MultipartWrite`, returning the
    /// result of completing the write as a future.
    fn collect_completed<Wr, Part>(self, writer: Wr) -> CollectCompleted<Self, Wr, Part>
    where
        Wr: MultipartWrite<Part>,
        Self: Stream<Item = Result<Part, Wr::Error>> + Sized,
    {
        CollectCompleted::new(self, writer)
    }
}
