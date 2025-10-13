use futures::stream::StreamExt as _;
use multipart_write::prelude::*;
use std::pin::Pin;
use std::task::{Context, Poll};

#[derive(Clone, Default, Debug)]
struct TestWriter {
    inner: Vec<usize>,
}

impl MultipartWrite<usize> for TestWriter {
    type Ret = usize;
    type Output = Vec<usize>;
    type Error = String;

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), String>> {
        Poll::Ready(Ok(()))
    }

    fn start_write(mut self: Pin<&mut Self>, part: usize) -> Result<usize, String> {
        self.as_mut().inner.push(part);
        Ok(part)
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_freeze(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        Poll::Ready(Ok(std::mem::take(&mut self.inner)))
    }
}

#[tokio::test]
async fn trait_futures() {
    let mut writer = TestWriter::default();
    writer.write_part(1).await.ok();
    writer.write_part(2).await.ok();
    writer.write_part(3).await.ok();
    writer.flush().await.ok();
    let out1 = writer.freeze().await.unwrap();

    writer.write_part(10).await.ok();
    writer.write_part(20).await.ok();
    writer.flush().await.ok();
    let out2 = writer.freeze().await.unwrap();

    assert_eq!(out1, vec![1, 2, 3]);
    assert_eq!(out2, vec![10, 20]);
}

#[tokio::test]
async fn writer_map() {
    let mut writer = TestWriter::default().map(|ns| ns.into_iter().fold(0, |a, n| a + n));
    for n in 1..=5 {
        writer.write_part(n).await.ok();
    }
    writer.flush().await.ok();
    let out = writer.freeze().await.unwrap();
    assert_eq!(out, 15);
}

#[tokio::test]
async fn writer_with() {
    let mut writer =
        TestWriter::default().with::<usize, String, _, _>(|n| futures::future::ready(Ok(n + 1)));
    writer.write_part(1).await.ok();
    writer.write_part(2).await.ok();
    writer.write_part(3).await.ok();
    writer.flush().await.ok();
    let out = writer.freeze().await.unwrap();

    assert_eq!(out, vec![2, 3, 4]);
}

#[tokio::test]
async fn stream_frozen() {
    let writer = TestWriter::default();
    let stream = futures::stream::iter(1..=10);
    let outputs = stream
        .frozen(writer, |n| n % 5 == 0)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(outputs.len(), 2);
}
