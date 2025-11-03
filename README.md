<h1 align="center">multipart-write</h1>
<br />
<div align="center">
  <!-- Version -->
  <a href="https://crates.io/crates/multipart-write">
    <img src="https://img.shields.io/crates/v/multipart-write.svg?style=flat-square"
    alt="Crates.io version" /></a>
  <!-- Docs -->
  <a href="https://docs.rs/multipart-write">
    <img src="https://img.shields.io/badge/docs-latest-blue.svg?style=flat-square" alt="docs.rs docs" /></a>
</div>

## Description

This crate contains the trait `MultipartWrite`, assorted implementations, and combinators.

A `MultipartWrite` is a similar interface to [`Sink`], except that writing an item or completing the
write both return values.

[Here][example] is a conceptual example of a `MultipartWrite`.

## Motivation

`Sink` is a useful API, but it is just that--a sink.  The end of a stream.
It's useful to have the backpressure mechanism that `poll_ready`/`start_send` enables, and it's nice
to have the flexibility that the shape of it provides in what kinds of things you can forward to it,
but it would be nice to use as an intermediate stage in a stream transformation, like a more powerful
[`StreamExt::buffered`][buffered].

The idea for `MultipartWrite` is to:
1. Have the same desirable properies as `Sink`.
2. Be able to be inserted at more locations in a stream computation.
3. Be useful in more cases by having a value returned when starting a write.
4. Be able to transform a stream into another stream, which is really just a more specific phrasing of 3.

[`Sink`]: https://docs.rs/crate/futures-sink/0.3.31
[example]: https://github.com/quasi-coherent/multipart-write/blob/master/examples/author.rs
[buffered]: https://docs.rs/futures/0.3.31/futures/stream/trait.StreamExt.html#method.buffered
