//! Using `MultipartWrite` with streams.
//!
//! This module contains the extension [`MultipartStreamExt`] that has adapters
//! for using a `MultipartWrite` with a stream.
use futures_core::stream::Stream;

use crate::{FusedMultipartWrite, MultipartWrite};

mod complete_with;
pub use complete_with::CompleteWith;

mod try_complete_when;
pub use try_complete_when::TryCompleteWhen;

impl<St: Stream> MultipartStreamExt for St {}

/// An extension trait for `Stream`s that provides combinators to use with
/// `MultipartWrite`rs.
pub trait MultipartStreamExt: Stream {
    /// Consumes a stream by passing to the provided `MultipartWrite`, returning
    /// the complete output of the writer in a future.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # futures::executor::block_on(async {
    /// use multipart_write::{
    ///     MultipartStreamExt as _, MultipartWriteExt as _, write,
    /// };
    ///
    /// let init: Vec<u8> = Vec::new();
    /// let writer = write::extend(init).then(|res| async move {
    ///     let vs = res?;
    ///     Ok(vs.iter().sum::<u8>())
    /// });
    ///
    /// let output = futures::stream::iter(1..=5)
    ///     .complete_with(writer)
    ///     .await;
    ///
    /// assert!(matches!(output, Ok(n) if n == 15));
    /// # });
    /// ```
    fn complete_with<Wr>(self, writer: Wr) -> CompleteWith<Self, Wr>
    where
        Wr: MultipartWrite<Self::Item>,
        Self: Sized,
    {
        CompleteWith::new(self, writer)
    }

    /// Transforms this stream into a stream of `Result`s returned by polling
    /// the writer for completion.
    ///
    /// `TryCompleteWhen` does this by writing the items as parts to the writer
    /// until the given closure evaluates to `true`, which triggers the writer
    /// to produce the complete output result.
    ///
    /// Note the stronger requirement of [`FusedMultipartWrite`] on the writer
    /// type. It must be safe to continue using the writer after it produces
    /// the next completed item for the stream, so prior to this it must check
    /// that the inner writer has not terminated. If either the stream or
    /// the writer are terminated, the stream is ended.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # futures::executor::block_on(async {
    /// use std::sync::Arc;
    /// use std::sync::atomic::{AtomicU8, Ordering};
    ///
    /// use futures::stream::{self, TryStreamExt as _};
    /// use multipart_write::{
    ///     MultipartStreamExt as _, MultipartWriteExt as _, write,
    /// };
    ///
    /// // The associated `Recv` type for the `FromExtend` writer is (), so
    /// // there's nothing to decide when to stop and complete a part.  So in
    /// // this contrived example a counter is used to complete the writer every
    /// // third item in the stream.
    /// let counter = Arc::new(AtomicU8::new(1));
    ///
    /// let init: Vec<u8> = Vec::new();
    /// let writer = write::extend(init).then(|res| async move {
    ///     let vs = res?;
    ///     Ok(vs.iter().sum::<u8>())
    /// });
    ///
    /// let output = stream::iter(1..=10)
    ///     .try_complete_when(writer, |_| {
    ///         let cnt = Arc::clone(&counter);
    ///         let n = cnt.fetch_add(1, Ordering::SeqCst);
    ///         n % 3 == 0
    ///     })
    ///     .try_collect::<Vec<_>>()
    ///     .await
    ///     .unwrap();
    ///
    /// assert_eq!(output, vec![6, 15, 24, 10]);
    /// # });
    /// ```
    fn try_complete_when<Wr, F>(
        self,
        writer: Wr,
        f: F,
    ) -> TryCompleteWhen<Self, Wr, F>
    where
        Wr: FusedMultipartWrite<Self::Item>,
        F: FnMut(Wr::Recv) -> bool,
        Self: Sized,
    {
        TryCompleteWhen::new(self, writer, f)
    }
}
