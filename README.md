# multipart-write

This crate contains the trait `MultipartWrite` and assorted implementations
and convenience combinators.

A `MultipartWrite` is a similar interface to [`Sink`][futures-sink], except that
it allows returning a value when writing a part and closing the writer.

[futures-sink]: https://docs.rs/futures/0.3.31/futures/prelude/trait.Sink.html
