use futures::future::ready;
use futures::stream::{StreamExt as _, iter};
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

    fn start_send(mut self: Pin<&mut Self>, part: usize) -> Result<usize, String> {
        self.as_mut().inner.push(part);
        Ok(part)
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_complete(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        Poll::Ready(Ok(std::mem::take(&mut self.inner)))
    }
}

#[tokio::test]
async fn trait_futures() {
    let mut writer = TestWriter::default();
    writer.send(1).await.ok();
    writer.send(2).await.ok();
    writer.send(3).await.ok();
    writer.flush().await.ok();
    let out1 = writer.complete().await.unwrap();

    writer.send(10).await.ok();
    writer.send(20).await.ok();
    writer.flush().await.ok();
    let out2 = writer.complete().await.unwrap();

    assert_eq!(out1, vec![1, 2, 3]);
    assert_eq!(out2, vec![10, 20]);
}

#[tokio::test]
async fn writer_map() {
    let mut writer = TestWriter::default().map(|ns| ns.iter().sum::<usize>());
    for n in 1..=5_usize {
        writer.send(n).await.ok();
    }
    writer.complete().await.ok();
    let out = writer.complete().await.unwrap();
    assert_eq!(out, 15);
}

#[tokio::test]
async fn writer_with() {
    let mut writer = TestWriter::default().with(|n| ready(Ok::<usize, String>(n + 1_usize)));
    writer.send(1).await.ok();
    writer.send(2).await.ok();
    writer.send(3).await.ok();
    writer.flush().await.ok();
    let out = writer.complete().await.unwrap();

    assert_eq!(out, vec![2, 3, 4]);
}

#[tokio::test]
async fn map_writer_stream() {
    let writer = TestWriter::default();
    let stream = futures::stream::iter(1..=10);
    let mut outputs = stream
        .map_writer(writer, |ret| ret % 5 == 0)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(outputs.len(), 2);

    let out2 = outputs.pop().unwrap();
    let out1 = outputs.pop().unwrap();
    assert_eq!(out1, vec![1, 2, 3, 4, 5]);
    assert_eq!(out2, vec![6, 7, 8, 9, 10]);
}

#[tokio::test]
async fn collect_complete_stream() {
    let writer = TestWriter::default().fold_ret(String::new(), |mut acc, n| {
        acc += &n.to_string();
        acc
    });
    let (acc, out) = iter(1..=5).map(Ok).collect_complete(writer).await.unwrap();
    assert_eq!(acc, "12345".to_string());
    assert_eq!(out, vec![1, 2, 3, 4, 5]);
}
