use futures_util::future::{Ready, ready};
use futures_util::stream::{StreamExt as _, iter};
use multipart_write::FusedMultipartWrite;
use multipart_write::prelude::*;
use std::pin::Pin;
use std::task::{Context, Poll};

#[derive(Clone, Debug)]
struct TestWriter {
    inner: Vec<usize>,
    multiplier: usize,
    capacity: usize,
}

impl Default for TestWriter {
    fn default() -> Self {
        Self {
            inner: Vec::new(),
            multiplier: 1,
            capacity: usize::MAX,
        }
    }
}

impl TestWriter {
    fn new(multiplier: usize) -> Self {
        Self {
            multiplier,
            ..Default::default()
        }
    }

    fn capacity(self, capacity: usize) -> Self {
        Self { capacity, ..self }
    }
}

impl MultipartWrite<usize> for TestWriter {
    type Ret = usize;
    type Output = Vec<usize>;
    type Error = String;

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), String>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(mut self: Pin<&mut Self>, part: usize) -> Result<usize, String> {
        let m = self.multiplier;
        self.as_mut().inner.push(m * part);
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

impl FusedMultipartWrite<usize> for TestWriter {
    fn is_terminated(&self) -> bool {
        self.inner.len() >= self.capacity
    }
}

#[tokio::test]
async fn trait_futures() {
    let mut writer = TestWriter::default();
    writer.send_part(1).await.unwrap();
    writer.send_part(2).await.unwrap();
    writer.send_part(3).await.unwrap();
    writer.flush().await.unwrap();
    let out1 = writer.complete().await.unwrap();

    writer.send_part(10).await.unwrap();
    writer.send_part(20).await.unwrap();
    writer.flush().await.unwrap();
    let out2 = writer.complete().await.unwrap();

    assert_eq!(out1, vec![1, 2, 3]);
    assert_eq!(out2, vec![10, 20]);
}

#[tokio::test]
async fn writer_map() {
    let mut writer = TestWriter::default().map(|ns| ns.iter().sum::<usize>());
    for n in 1..=5_usize {
        writer.send_part(n).await.unwrap();
    }
    writer.complete().await.unwrap();
    let out = writer.complete().await.unwrap();
    assert_eq!(out, 15);
}

#[tokio::test]
async fn writer_with() {
    let mut writer = TestWriter::default().with(|n| ready(Ok::<usize, String>(n + 1_usize)));
    writer.send_part(1).await.unwrap();
    writer.send_part(2).await.unwrap();
    writer.send_part(3).await.unwrap();
    writer.flush().await.unwrap();
    let out = writer.complete().await.unwrap();

    assert_eq!(out, vec![2, 3, 4]);
}

#[tokio::test]
async fn feed_multipart_write_stream() {
    let writer = TestWriter::default().capacity(100);
    let mut outputs = iter(1..=10)
        .feed_multipart_write(writer, |ret| ret % 5 == 0)
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
async fn write_complete_stream() {
    let writer = TestWriter::default().fold_ret(String::new(), |mut acc, n| {
        acc += &n.to_string();
        acc
    });
    let (acc, out) = iter(1..=5).write_complete(writer).await.unwrap();
    assert_eq!(acc, "12345".to_string());
    assert_eq!(out, vec![1, 2, 3, 4, 5]);
}

struct TestState(usize);

impl Default for TestState {
    fn default() -> Self {
        Self(1)
    }
}

impl TestState {
    fn bootstrap(&mut self) -> Ready<Result<Option<TestWriter>, String>> {
        self.0 += 1;
        let writer = TestWriter::new(self.0);
        ready(Ok(Some(writer)))
    }
}

#[tokio::test]
async fn bootstrapped_writer() {
    let writer = TestWriter::default().bootstrapped(TestState::default(), TestState::bootstrap);
    let mut output = iter(1..=5)
        .feed_multipart_write(writer, |ret| ret % 3 == 0)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(output.len(), 2);

    let out2 = output.pop().unwrap();
    let out1 = output.pop().unwrap();
    assert_eq!(out1, vec![1, 2, 3]);
    assert_eq!(out2, vec![8, 10]);
}
