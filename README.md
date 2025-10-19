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

[`Sink`]: https://docs.rs/crate/futures-sink/0.3.31
