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

This crate contains the trait `MultipartWrite`, assorted implementations,
and combinators.

A `MultipartWrite` is a similar interface to [`Sink`], except that writing
an item or completing the write both return values.

See a conceptual example of a `MultipartWrite` [here][example].

# Motivation

`Sink` is a useful API, but it is just that--a sink.  The end of a stream.
It's useful to have the backpressure mechanism that `poll_ready`/`start_send`
enables while being able to act as a stream itself and produce items that
can be forwarded for more processing.

[example]: https://github.com/quasi-coherent/multipart-write/blob/2cfd8bab323132ba3c0caa9f31b33b45d9faf8c1/examples/author.rs
[`Sink`]: https://docs.rs/crate/futures-sink/0.3.31
