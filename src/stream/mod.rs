//! `MultipartWrite`rs compatible with [`Stream`].
use crate::MultipartWrite;

use futures_core::stream::Stream;

mod feed_multipart_write;
pub use feed_multipart_write::FeedMultipartWrite;

mod write_complete;
pub use write_complete::WriteComplete;

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
    fn write_complete<Wr>(self, writer: Wr) -> WriteComplete<Self, Wr>
    where
        Wr: MultipartWrite<Self::Item>,
        Self: Sized,
    {
        WriteComplete::new(self, writer)
    }
}
