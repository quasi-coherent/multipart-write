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

`Sink` is a useful API, but it is just that: a sink, the end of a stream.
It's valuable to have the backpressure mechanism that `poll_ready` combined with `start_send` enables,
and it's nice to have the flexibility that the shape of `Sink` provides in what kinds of values you can
send with it.  Sort of like a more capable [`StreamExt::buffered`].

The idea for `MultipartWrite` is to:
1. Have those same desirable properies: backpressure and generic input type.
2. Be able to be inserted earlier in a stream computation.
3. Replace `Sink` when the use case would need a value returned by sending to it or closing it.
4. Transform a stream by writing it in parts, which is somewhat of a specific rephrasing of the second and
   third points.

[`Sink`]: https://docs.rs/crate/futures-sink/0.3.31
[example]: https://github.com/quasi-coherent/multipart-write/blob/master/examples/author.rs
[buffered]: https://docs.rs/futures/0.3.31/futures/stream/trait.StreamExt.html#method.buffered
