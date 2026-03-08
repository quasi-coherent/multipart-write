//! `MultipartWrite` combinators.
//!
//! This module contains the trait [`MultipartWriteExt`], which provides
//! adapters for chaining and composing writers.
use crate::{
    BoxFusedMultipartWrite, BoxMultipartWrite, FusedMultipartWrite,
    LocalBoxFusedMultipartWrite, LocalBoxMultipartWrite, MultipartWrite,
};

use futures_core::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

mod buffered;
pub use buffered::Buffered;

mod complete;
pub use complete::Complete;

mod extend;
pub use extend::{Extend, extend, extend_default};

mod fanout;
pub use fanout::Fanout;

mod feed;
pub use feed::Feed;

mod filter_map_part;
pub use filter_map_part::FilterMapPart;

mod filter_part;
pub use filter_part::FilterPart;

mod flush;
pub use flush::Flush;

mod fold_sent;
pub use fold_sent::FoldSent;

mod for_each_recv;
pub use for_each_recv::ForEachRecv;

mod fuse;
pub use fuse::Fuse;

mod lift;
pub use lift::Lift;

mod map_err;
pub use map_err::MapErr;

mod map_ok;
pub use map_ok::MapOk;

mod map_sent;
pub use map_sent::MapSent;

mod ready_part;
pub use ready_part::ReadyPart;

mod send_flush;
pub use send_flush::SendFlush;

mod then;
pub use then::Then;

impl<Wr: MultipartWrite<Part>, Part> MultipartWriteExt<Part> for Wr {}

/// An extension trait for `MultipartWrite` providing a variety of convenient
/// combinator functions.
pub trait MultipartWriteExt<Part>: MultipartWrite<Part> {
    /// Wrap this writer in a `Box`, pinning it.
    fn boxed<'a>(
        self,
    ) -> BoxMultipartWrite<'a, Part, Self::Recv, Self::Output, Self::Error>
    where
        Self: Sized + Send + 'a,
    {
        Box::pin(self)
    }

    /// Wrap this writer, which additionally has conditions making it a
    /// [`FusedMultipartWrite`], in a `Box`, pinning it.
    fn box_fused<'a>(
        self,
    ) -> BoxFusedMultipartWrite<'a, Part, Self::Recv, Self::Output, Self::Error>
    where
        Self: Sized + Send + FusedMultipartWrite<Part> + 'a,
    {
        Box::pin(self)
    }

    /// Wrap this writer, which additionally has conditions making it a
    /// [`FusedMultipartWrite`], in a `Box`, pinning it.
    ///
    /// Similar to `box_fused` but without the `Send` requirement.
    fn box_fused_local<'a>(
        self,
    ) -> LocalBoxFusedMultipartWrite<
        'a,
        Part,
        Self::Recv,
        Self::Output,
        Self::Error,
    >
    where
        Self: Sized + FusedMultipartWrite<Part> + 'a,
    {
        Box::pin(self)
    }

    /// Wrap this writer in a `Box`, pinning it.
    ///
    /// Similar to `boxed` but without the `Send` requirement.
    fn boxed_local<'a>(
        self,
    ) -> LocalBoxMultipartWrite<'a, Part, Self::Recv, Self::Output, Self::Error>
    where
        Self: Sized + 'a,
    {
        Box::pin(self)
    }

    /// Adds a fixed size buffer to the current writer.
    ///
    /// The resulting `MultipartWrite` will buffer up to `capacity` items when
    /// the underlying writer is not able to accept new parts.
    ///
    /// The values returned when the underlying writer has received a part are
    /// also accumulated and returned in batch.
    fn buffered(
        self,
        capacity: impl Into<Option<usize>>,
    ) -> Buffered<Self, Part>
    where
        Self: Sized,
    {
        assert_writer::<
            Part,
            Option<Vec<Self::Recv>>,
            Self::Error,
            Self::Output,
            _,
        >(Buffered::new(self, capacity.into().unwrap_or_default()))
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
    ///
    /// # Examples
    ///
    /// ```rust
    /// # futures::executor::block_on(async {
    /// use multipart_write::{MultipartWriteExt as _, write};
    ///
    /// let init: Vec<u8> = Vec::new();
    /// let wr1 = write::extend(init.clone());
    /// let wr2 = write::extend(init);
    ///
    /// let mut writer = wr1.fanout(wr2);
    /// writer.send_flush(1).await.unwrap();
    /// writer.send_flush(2).await.unwrap();
    /// writer.send_flush(3).await.unwrap();
    /// let out = writer.complete().await.unwrap();
    ///
    /// assert_eq!(out, (vec![1, 2, 3], vec![1, 2, 3]));
    /// # })
    /// ```
    fn fanout<U>(self, other: U) -> Fanout<Self, U, Part>
    where
        Part: Clone,
        U: MultipartWrite<Part, Error = Self::Error>,
        Self: Sized,
    {
        assert_writer::<
            Part,
            (Self::Recv, U::Recv),
            Self::Error,
            (Self::Output, U::Output),
            _,
        >(Fanout::new(self, other))
    }

    /// A future that completes after the given part has been received by the
    /// writer.
    ///
    /// Unlike `send_flush`, the returned future does not flush the writer.  It
    /// is the caller's responsibility to ensure all pending items are processed
    /// by calling `flush` or `complete`.
    fn feed(&mut self, part: Part) -> Feed<'_, Self, Part>
    where
        Self: Unpin,
    {
        Feed::new(self, part)
    }

    /// Apply a filter to this writer's parts, returning a new writer with the
    /// same output.
    ///
    /// The return type of this writer is `Option<Self::Recv>` and is `None`
    /// when the part did not pass the filter.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # futures::executor::block_on(async {
    /// use multipart_write::{MultipartWriteExt, write};
    ///
    /// let init: Vec<u8> = Vec::new();
    /// let mut writer = write::extend(init).filter_part(|n| n % 2 == 0);
    ///
    /// let r1 = writer.send_flush(1).await.unwrap();
    /// let r2 = writer.send_flush(2).await.unwrap();
    /// let r3 = writer.send_flush(3).await.unwrap();
    /// let out = writer.complete().await.unwrap();
    ///
    /// assert!(r1.is_none() && r2.is_some() && r3.is_none());
    /// assert_eq!(out, vec![2]);
    /// # })
    /// ```
    fn filter_part<F>(self, f: F) -> FilterPart<Self, Part, F>
    where
        F: FnMut(&Part) -> bool,
        Self: Sized,
    {
        assert_writer::<Part, Option<Self::Recv>, Self::Error, Self::Output, _>(
            FilterPart::new(self, f),
        )
    }

    /// Attempt to map the input to a part for this writer, filtering out the
    /// inputs where the mapping returns `None`.
    ///
    /// The return type of this writer is `Option<Self::Recv>` and is `None`
    /// when the provided closure returns `None`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # futures::executor::block_on(async {
    /// use multipart_write::{MultipartWriteExt as _, write};
    ///
    /// let init: Vec<String> = Vec::new();
    /// let mut writer = write::extend(init).filter_map_part(|n: u8| {
    ///     if n % 2 == 0 { Some(n.to_string()) } else { None }
    /// });
    ///
    /// let r1 = writer.send_flush(1).await.unwrap();
    /// let r2 = writer.send_flush(2).await.unwrap();
    /// let r3 = writer.send_flush(3).await.unwrap();
    /// let out = writer.complete().await.unwrap();
    ///
    /// assert!(r1.is_none() && r2.is_some() && r3.is_none());
    /// assert_eq!(out, vec!["2".to_string()]);
    /// # })
    /// ```
    fn filter_map_part<P, F>(self, f: F) -> FilterMapPart<Self, Part, P, F>
    where
        F: FnMut(P) -> Option<Part>,
        Self: Sized,
    {
        assert_writer::<P, Option<Self::Recv>, Self::Error, Self::Output, _>(
            FilterMapPart::new(self, f),
        )
    }

    /// A future that completes when the underlying writer has been flushed.
    fn flush(&mut self) -> Flush<'_, Self, Part>
    where
        Self: Unpin,
    {
        Flush::new(self)
    }

    /// Accumulate the values returned by starting a send, returning it with the
    /// output.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # futures::executor::block_on(async {
    /// use multipart_write::{MultipartWriteExt as _, write};
    ///
    /// let init: Vec<u8> = Vec::new();
    /// let mut writer = write::extend(init).fold_sent(0, |n, _| n + 1);
    ///
    /// let r1 = writer.send_flush(1).await.unwrap();
    /// let r2 = writer.send_flush(2).await.unwrap();
    /// let r3 = writer.send_flush(3).await.unwrap();
    /// let out = writer.complete().await.unwrap();
    ///
    /// assert_eq!(out, (3, vec![1, 2, 3]));
    /// # })
    /// ```
    fn fold_sent<T, F>(self, id: T, f: F) -> FoldSent<Self, T, F, Part>
    where
        F: FnMut(T, &Self::Recv) -> T,
        Self: Sized,
    {
        assert_writer::<Part, Self::Recv, Self::Error, (T, Self::Output), _>(
            FoldSent::new(self, id, f),
        )
    }

    /// Evaluate the given async closure on the associated `Self::Recv` for this
    /// writer.
    ///
    /// The result is a new writer that has all of the same properties as this
    /// writer, except that `poll_ready` will not accept the next part until the
    /// future returned by evaluating `F` on the return value resolves.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # futures::executor::block_on(async {
    /// use std::sync::Arc;
    /// use std::sync::atomic::{AtomicU8, Ordering};
    ///
    /// use multipart_write::{MultipartWriteExt as _, write};
    ///
    /// let counter = Arc::new(AtomicU8::new(1));
    ///
    /// // `extend` has no return type, so `map_sent` makes one for the
    /// // demonstration.
    /// let init: Vec<u8> = Vec::new();
    /// let mut writer = write::extend(init)
    ///     .map_sent(|_| {
    ///         let cnt = Arc::clone(&counter);
    ///         let n = cnt.fetch_add(1, Ordering::SeqCst);
    ///         n
    ///     })
    ///     .for_each_recv(|n| {
    ///         println!("{n} parts written");
    ///         futures::future::ready(())
    ///     });
    ///
    /// let r1 = writer.send_flush(1).await.unwrap();
    /// let r2 = writer.send_flush(2).await.unwrap();
    /// let r3 = writer.send_flush(3).await.unwrap();
    /// let out = writer.complete().await.unwrap();
    ///
    /// assert_eq!(out, vec![1, 2, 3]);
    /// # })
    /// ```
    fn for_each_recv<Fut, F>(self, f: F) -> ForEachRecv<Self, Part, Fut, F>
    where
        Self: Sized,
        Self::Recv: Clone,
        F: FnMut(Self::Recv) -> Fut,
        Fut: Future<Output = ()>,
    {
        assert_writer::<Part, Self::Recv, Self::Error, Self::Output, _>(
            ForEachRecv::new(self, f),
        )
    }

    /// Returns a new writer that fuses according to the provided closure.
    ///
    /// The resulting writer wraps both `Self::Recv` and `Self::Output` in
    /// an `Option` and is guaranteed to both output and return `Ok(None)`
    /// when called after becoming fused.
    fn fuse<F>(self, f: F) -> Fuse<Self, Part, F>
    where
        F: FnMut(&Self::Output) -> bool,
        Self: Sized,
    {
        assert_writer::<
            Part,
            Option<Self::Recv>,
            Self::Error,
            Option<Self::Output>,
            _,
        >(Fuse::new(self, f))
    }

    /// Produce the parts for this writer from the output of another writer.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # futures::executor::block_on(async {
    /// use multipart_write::{MultipartWriteExt as _, write};
    ///
    /// let init: Vec<u8> = Vec::new();
    /// let wr = write::extend(init.clone()).map_ok(|vs| vs.iter().sum::<u8>());
    /// let mut writer = write::extend(init).lift(wr);
    ///
    /// // We use `feed` and not `send_flush` because `send_flush` will complete
    /// // the outer writer and write its output to the inner writer after each
    /// // send, which is not what we want the example to show.
    /// writer.feed(1).await.unwrap();
    /// writer.feed(2).await.unwrap();
    ///
    /// // Flush the writer manually, which now completes the outer writer and
    /// // writes its output, the sum of the parts written, to the inner writer.
    /// writer.flush().await.unwrap();
    ///
    /// writer.feed(3).await.unwrap();
    /// writer.feed(4).await.unwrap();
    /// writer.feed(5).await.unwrap();
    /// let out = writer.complete().await.unwrap();
    ///
    /// assert_eq!(out, vec![3, 12]);
    /// # })
    /// ```
    fn lift<U, P>(self, other: U) -> Lift<Self, U, P, Part>
    where
        Self: Sized,
        Self::Error: From<U::Error>,
        U: MultipartWrite<P, Output = Part>,
    {
        assert_writer::<P, U::Recv, Self::Error, Self::Output, _>(Lift::new(
            self, other,
        ))
    }

    /// Map this writer's return type to a different value, returning a new
    /// multipart writer with the given return type.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # futures::executor::block_on(async {
    /// use multipart_write::{MultipartWriteExt as _, write};
    ///
    /// let init: Vec<u8> = Vec::new();
    /// let mut writer = write::extend(init).map_sent(|_| "OK");
    ///
    /// let r1 = writer.send_flush(1).await.unwrap();
    /// let r2 = writer.send_flush(2).await.unwrap();
    /// let r3 = writer.send_flush(3).await.unwrap();
    /// let out = writer.complete().await.unwrap();
    ///
    /// assert_eq!(vec![r1, r2, r3], vec!["OK", "OK", "OK"]);
    /// assert_eq!(out, vec![1, 2, 3]);
    /// # })
    /// ```
    fn map_sent<R, F>(self, f: F) -> MapSent<Self, Part, R, F>
    where
        F: FnMut(Self::Recv) -> R,
        Self: Sized,
    {
        assert_writer::<Part, R, Self::Error, Self::Output, _>(MapSent::new(
            self, f,
        ))
    }

    /// Map this writer's error type to a different value, returning a new
    /// multipart writer with the given error type.
    fn map_err<E, F>(self, f: F) -> MapErr<Self, Part, E, F>
    where
        F: FnMut(Self::Error) -> E,
        Self: Sized,
    {
        assert_writer::<Part, Self::Recv, E, Self::Output, _>(MapErr::new(
            self, f,
        ))
    }

    /// Map this writer's output type to a different type, returning a new
    /// multipart writer with the given output type.
    fn map_ok<T, F>(self, f: F) -> MapOk<Self, Part, T, F>
    where
        F: FnMut(Self::Output) -> T,
        Self: Sized,
    {
        assert_writer::<Part, Self::Recv, Self::Error, T, _>(MapOk::new(
            self, f,
        ))
    }

    /// A convenience method for calling [`MultipartWrite::poll_ready`] on
    /// [`Unpin`] writer types.
    #[must_use = "futures do nothing unless polled"]
    fn poll_ready_unpin(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>>
    where
        Self: Unpin,
    {
        Pin::new(self).poll_ready(cx)
    }

    /// A convenience method for calling [`MultipartWrite::poll_flush`] on
    /// [`Unpin`] writer types.
    #[must_use = "futures do nothing unless polled"]
    fn poll_flush_unpin(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>>
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

    /// Provide a part to this writer in the output of a future.
    ///
    /// The result is a new writer over the type `U` that passes each value
    /// through the function `f`, resolving the output, and sending it to the
    /// inner writer.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # futures::executor::block_on(async {
    /// use multipart_write::{MultipartWriteExt as _, write};
    ///
    /// let init: Vec<u8> = Vec::new();
    /// let mut writer = write::extend(init)
    ///     .ready_part(|n: u8| futures::future::ready(Ok(n + 1)));
    ///
    /// writer.send_flush(1).await.unwrap();
    /// writer.send_flush(2).await.unwrap();
    /// writer.send_flush(3).await.unwrap();
    /// let out = writer.complete().await.unwrap();
    ///
    /// assert_eq!(out, vec![2, 3, 4]);
    /// # })
    /// ```
    fn ready_part<P, Fut, F>(self, f: F) -> ReadyPart<Self, Part, P, Fut, F>
    where
        F: FnMut(P) -> Fut,
        Fut: Future<Output = Result<Part, Self::Error>>,
        Self: Sized,
    {
        assert_writer::<P, (), Self::Error, Self::Output, _>(ReadyPart::new(
            self, f,
        ))
    }

    /// A future that completes when a part has been fully processed into the
    /// writer, including flushing.
    fn send_flush(&mut self, part: Part) -> SendFlush<'_, Self, Part>
    where
        Self: Unpin,
    {
        SendFlush::new(self, part)
    }

    /// Asynchronously map the result of completing this writer to a different
    /// result.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # futures::executor::block_on(async {
    /// use multipart_write::{MultipartWriteExt as _, write};
    ///
    /// let init: Vec<u8> = Vec::new();
    /// let mut writer = write::extend(init).then(|res| {
    ///     futures::future::ready(res.map(|vs| vs.iter().sum::<u8>()))
    /// });
    ///
    /// writer.send_flush(1).await.unwrap();
    /// writer.send_flush(2).await.unwrap();
    /// writer.send_flush(3).await.unwrap();
    /// let out = writer.complete().await.unwrap();
    ///
    /// assert_eq!(out, 6);
    /// # });
    /// ```
    fn then<T, Fut, F>(self, f: F) -> Then<Self, Part, T, Fut, F>
    where
        F: FnMut(Result<Self::Output, Self::Error>) -> Fut,
        Fut: Future<Output = Result<T, Self::Error>>,
        Self: Sized,
    {
        assert_writer::<Part, Self::Recv, Self::Error, T, _>(Then::new(self, f))
    }
}

fn assert_writer<Part, R, E, T, Wr>(wr: Wr) -> Wr
where
    Wr: MultipartWrite<Part, Recv = R, Error = E, Output = T>,
{
    wr
}
