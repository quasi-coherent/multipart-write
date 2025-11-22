use futures_util::future;
use futures_util::stream::{StreamExt as _, iter};
use multipart_write::stream::MultipartStreamExt as _;
use multipart_write::{FusedMultipartWrite, MultipartWrite, MultipartWriteExt as _};
use std::pin::Pin;
use std::task::{Context, Poll};

#[derive(Clone, Debug)]
struct TestWriter {
    inner: Vec<usize>,
    multiplier: usize,
    completed: usize,
    max_completed: Option<usize>,
}

impl Default for TestWriter {
    fn default() -> Self {
        Self {
            inner: Vec::new(),
            multiplier: 1,
            completed: 0,
            max_completed: None,
        }
    }
}

impl TestWriter {
    fn new(multiplier: usize) -> Self {
        Self {
            multiplier,
            inner: Vec::new(),
            completed: 0,
            max_completed: None,
        }
    }

    fn max_completed(mut self, max: usize) -> Self {
        self.max_completed = Some(max);
        self
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
        self.completed += 1;
        Poll::Ready(Ok(std::mem::take(&mut self.inner)))
    }
}

impl FusedMultipartWrite<usize> for TestWriter {
    fn is_terminated(&self) -> bool {
        self.max_completed.is_some_and(|n| n <= self.completed)
    }
}

#[derive(Debug, Clone, Default)]
struct OtherTestWriter(Vec<String>);

impl MultipartWrite<Vec<usize>> for OtherTestWriter {
    type Ret = usize;
    type Output = String;
    type Error = String;

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), String>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(mut self: Pin<&mut Self>, part: Vec<usize>) -> Result<usize, String> {
        let n: usize = part.iter().sum();
        self.as_mut().0.push(n.to_string());
        Ok(self.0.len())
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_complete(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        let out = self.0.join(",");
        Poll::Ready(Ok(out))
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
    let mut writer = TestWriter::default().map_ok(|ns| ns.iter().sum::<usize>());
    for n in 1..=5_usize {
        writer.send_part(n).await.unwrap();
    }
    writer.flush().await.unwrap();
    let out = writer.complete().await.unwrap();
    assert_eq!(out, 15);
}

#[tokio::test]
async fn writer_with() {
    let mut writer = TestWriter::new(2)
        .with(|x: String| async move { Ok::<usize, String>(x.len()) })
        .boxed();
    writer.send_part("abc".to_string()).await.unwrap();
    writer.send_part("d".to_string()).await.unwrap();
    writer.send_part("ef".to_string()).await.unwrap();
    writer.flush().await.unwrap();
    let out = writer.complete().await.unwrap();

    assert_eq!(out, vec![6, 2, 4]);
}

#[tokio::test]
async fn assembled_stream() {
    let writer = TestWriter::default();
    let mut outputs = iter(1..=12)
        .assembled(writer, |ret| ret % 5 == 0)
        .filter_map(|res| future::ready(res.ok()))
        .collect::<Vec<_>>()
        .await;
    assert_eq!(outputs.len(), 3);

    let out3 = outputs.pop().unwrap();
    let out2 = outputs.pop().unwrap();
    let out1 = outputs.pop().unwrap();
    assert_eq!(out1, vec![1, 2, 3, 4, 5]);
    assert_eq!(out2, vec![6, 7, 8, 9, 10]);
    assert_eq!(out3, vec![11, 12]);
}

#[tokio::test]
async fn assemble_future() {
    let writer = TestWriter::default().fold_ret(String::new(), |mut acc, n| {
        acc += &n.to_string();
        acc
    });
    let (acc, out) = iter(1..=5).assemble(writer).await.unwrap();
    assert_eq!(acc, "12345".to_string());
    assert_eq!(out, vec![1, 2, 3, 4, 5]);
}

#[tokio::test]
async fn skip_last_complete_if_empty() {
    let writer = TestWriter::default();
    let outputs = iter(1..=10)
        .assembled(writer, |ret| ret % 5 == 0)
        .filter_map(|res| future::ready(res.ok()))
        .collect::<Vec<_>>()
        .await;
    assert_eq!(outputs.len(), 2);
}

#[tokio::test]
async fn stream_ends_on_fused_writer() {
    let writer = TestWriter::default().max_completed(4);
    let mut outputs = iter(1..)
        .assembled(writer, |ret| ret % 2 == 0)
        .filter_map(|res| future::ready(res.ok()))
        .collect::<Vec<_>>()
        .await;
    assert_eq!(outputs.pop(), Some(vec![7, 8]));
    assert_eq!(outputs.pop(), Some(vec![5, 6]));
    assert_eq!(outputs.pop(), Some(vec![3, 4]));
    assert_eq!(outputs.pop(), Some(vec![1, 2]));
    assert!(outputs.pop().is_none())
}

#[tokio::test]
async fn lift_writer() {
    let writer = OtherTestWriter::default().lift(TestWriter::default());
    let out = iter(1..=5).assemble(writer).await.unwrap();
    assert_eq!(&out, "15");
}
