# multipart-write

This crate contains the trait [`MultipartWrite`] and assorted implementations
convenience combinators.

A `MultipartWrite` is a similar interface to [`Sink`], except that writing
an item or completing the write both return values.

[`Sink`]: https://docs.rs/crate/futures-sink/0.3.31
