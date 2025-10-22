//! This example centers on an [`Author`].
//!
//! The author writes books.  The books are created by writing pages until some
//! conditions are met.  This is modeled as a `MultipartWrite` by having the part
//! be a line on a page, the flushing behavior be finishing a page and starting a
//! new page, and the completion of a writer be finishing the book.
use futures_util::{Future, Stream, StreamExt, future, stream};
use multipart_write::prelude::*;
use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use std::ops::Deref;
use std::pin::Pin;
use std::task::{Context, Poll};

#[tokio::main]
async fn main() -> Result<(), String> {
    let example = Example;
    let short_story = example.short_story().await?;
    println!("{short_story}");

    let backwards = example.backwards().await?;
    println!("{backwards}");

    let books: Vec<Book> = example
        .books()
        .filter_map(|output| future::ready(output.ok()))
        .collect::<Vec<_>>()
        .await;
    books.into_iter().for_each(|book| println!("{book}"));

    Ok(())
}

struct Example;
impl Example {
    /// A short story has 10 pages, each having 10 lines.  We'll make a short
    /// story in `Book` form from a stream (...of consciousness?).
    ///
    /// The book is returned in a future.  The future represents the computation
    /// of writing items from the stream, flushing when necessary, then
    fn short_story(&self) -> impl Future<Output = Result<Book, String>> {
        let author = Author::new(10);

        stream::iter(Narrative)
            .take(10 * 10)
            .map(Line)
            .write_complete(author)
    }

    /// Here is the same short story except every line is backwards now.
    /// The combinator `map_part` pre-composes the writer with a synchronous
    /// function, returning a new writer on the output type.
    ///
    /// The combinator `with` is similar except the function returns a future.
    fn backwards(&self) -> impl Future<Output = Result<Book, String>> {
        let rohtua =
            Author::new(10).map_part(|line: String| Ok(Line(line.chars().rev().collect())));

        stream::iter(Narrative).take(10 * 10).write_complete(rohtua)
    }

    /// A `MultipartWrite` can also be turned into a stream.
    ///
    /// This stream is a prolific author, who writes books until there isn't any
    /// more to write, i.e., when the stream ends or the writer `is_terminated`
    /// according to `FusedMultipartWrite`.  But we'll limit each book to 100
    /// pages.
    ///
    /// The combinator that allows this is `feed_multipart_write`.  The closure
    /// determines when to produce the next item, or put differently, when to
    /// call `poll_close`.  This is where the 100 page per book limit is
    /// injected.
    fn books(&self) -> impl Stream<Item = Result<Book, String>> {
        let author = Author::new(10);

        stream::iter(Narrative)
            .take(3125)
            .map(Line)
            .feed_multipart_write(author, |state| state.page_number() >= 100)
    }
}

/// A story to tell in a book.
#[derive(Debug, Clone, Copy)]
struct Narrative;

impl Iterator for Narrative {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        Some("All work and no play make Jack a dull boy.".into())
    }
}

/// `Author` is an example of a typical multipart writer.
///
/// We repeatedly write `Line`s to a `Page`.  The `Page` has a configured line
/// limit.  When the limit is achieved, we `poll_flush` to finish the page and
/// add it to the `Book`, starting a new page.
///
/// When enough pages have been written, `poll_complete` to finish the `Book`.
struct Author {
    book: Book,
    page: Page,
    line_limit: usize,
    current_page: PageNumber,
}

impl Author {
    fn new(line_limit: usize) -> Self {
        Self {
            book: Book::default(),
            page: Page::new(line_limit),
            line_limit,
            current_page: PageNumber::default(),
        }
    }

    /// Finish a page, returning the page number of the new page.
    ///
    /// This returns an error if there was already a page with the current page
    /// number.
    fn new_page(&mut self) -> Result<(), String> {
        let finished_page = std::mem::replace(&mut self.page, Page::new(self.line_limit));
        self.book.write_page(self.current_page, finished_page)?;
        self.current_page.new_page();
        Ok(())
    }

    fn book_state(&self, lines_written: usize) -> BookState {
        BookState {
            page_number: self.current_page,
            lines_written,
        }
    }
}

/// The current progress of the page is the return type of this writer.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
struct BookState {
    page_number: PageNumber,
    lines_written: usize,
}

impl BookState {
    fn page_number(&self) -> u64 {
        let PageNumber(n) = self.page_number;
        n
    }
}

impl MultipartWrite<Line> for Author {
    type Ret = BookState;
    type Output = Book;
    type Error = String;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        if self.page.line_count() >= self.line_limit {
            self.new_page()?;
        }
        Poll::Ready(Ok(()))
    }

    fn start_send(mut self: Pin<&mut Self>, line: Line) -> Result<Self::Ret, Self::Error> {
        let lines_written = self.page.write_line(line)?;
        Ok(self.book_state(lines_written))
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(self.new_page())
    }

    fn poll_complete(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        if self.page.line_count() > 0 {
            self.new_page()?;
        }
        let new_book = self.book.start_next();
        let book = std::mem::replace(&mut self.book, new_book);
        self.current_page = PageNumber::default();
        Poll::Ready(Ok(book))
    }
}

// We need this to be able to use it in a stream.
impl FusedMultipartWrite<Line> for Author {
    fn is_terminated(&self) -> bool {
        false
    }
}

/// A line is some text.
#[derive(Debug, Clone)]
struct Line(String);

impl Deref for Line {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

/// A `Page` is a collection of `Line`s.
#[derive(Debug, Clone, Default)]
struct Page(Vec<Line>);

impl Page {
    /// A new page with some line limit.
    fn new(limit: usize) -> Self {
        Self(Vec::with_capacity(limit))
    }

    /// Current line count.
    fn line_count(&self) -> usize {
        self.0.len()
    }

    /// Write a line, returning the number of lines the page currently has.
    fn write_line(&mut self, line: Line) -> Result<usize, String> {
        self.0.push(line);
        Ok(self.0.len())
    }
}

/// Ordering of the `Page`s in a book.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
struct PageNumber(u64);

impl Default for PageNumber {
    fn default() -> Self {
        Self(1)
    }
}

impl PageNumber {
    fn new_page(&mut self) {
        self.0 += 1;
    }
}

impl Display for PageNumber {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A `Book` has an edition and `Page`s.
#[derive(Debug, Clone)]
struct Book(usize, HashMap<PageNumber, Page>);

impl Default for Book {
    fn default() -> Self {
        Self(1, HashMap::default())
    }
}

impl Book {
    fn edition(&self) -> String {
        format!("Ed. {}", self.0)
    }

    fn start_next(&self) -> Book {
        Book(self.0 + 1, HashMap::default())
    }

    /// Write a page to the book.
    ///
    /// This returns an error if the book already has a page `page_number`.
    fn write_page(&mut self, page_number: PageNumber, page: Page) -> Result<(), String> {
        if self.1.contains_key(&page_number) {
            return Err(format!("page {page_number} already exists"));
        }
        self.1.insert(page_number, page);
        Ok(())
    }
}

impl Display for Book {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let edition = self.edition();
        self.1.iter().try_for_each(|(page_number, page)| {
            writeln!(f, "Book, {edition}")?;
            writeln!(f, "    Page {page_number}:")?;
            for line in &page.0 {
                writeln!(f, "{}", &**line)?;
            }
            Ok(())
        })
    }
}
