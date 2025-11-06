//! This example centers on an `Author`.
//!
//! The author writes books.  The books are created by writing lines to a page
//! until some configured limit, and pages to a book until some condition is met.
//!
//! This is modeled as a `MultipartWrite` by having the part be a line on a page,
//! the return value of starting a write be the page number and current line
//! count, the flushing behavior be finishing a page and starting a new page, and
//! the completion of the writer be finishing the book.
use futures_core::ready;
use futures_util::{Future, Stream, StreamExt, future, stream};
use multipart_write::prelude::*;
use std::collections::BTreeMap;
use std::fmt::{self, Display, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

// Stream length.
const TOTAL_LINES: usize = 625;

#[tokio::main]
async fn main() -> Result<(), String> {
    let short_story = Narrative::stream(TOTAL_LINES)
        .collect_writer(Author::default())
        .await
        .unwrap();
    println!("{short_story}");
    println!("=========================");

    let short_story_reversed = Narrative::stream(TOTAL_LINES)
        .collect_writer(Author::default().reverse())
        .await
        .unwrap();
    println!("{short_story_reversed}");
    println!("=========================");

    let books: Vec<Book> = Narrative::stream(TOTAL_LINES)
        .write_until(Author::default(), |book_state| {
            book_state.page_number() >= 25
        })
        .filter_map(|res| future::ready(res.ok()))
        .collect()
        .await;
    books.into_iter().for_each(|book| println!("{book}"));
    println!("=========================");

    let french_books: Vec<Book> = Narrative::stream(TOTAL_LINES)
        .write_until(Author::default().into_french(), |book_state| {
            book_state.page_number() >= 25
        })
        .filter_map(|res| future::ready(res.ok()))
        .collect()
        .await;
    french_books.into_iter().for_each(|book| println!("{book}"));
    println!("=========================");

    Ok(())
}

impl Author {
    /// This `MultipartWrite` augments the original implementation by reversing
    /// the input text before sending to the writer.  The result is a new writer
    /// that writes the same books but every line is backwards.
    fn reverse(self) -> impl MultipartWrite<String, Ret = (), Output = Book, Error = String> {
        self.with(|line: String| async move {
            let rev: String = line.chars().rev().collect();
            Ok::<String, String>(rev)
        })
    }

    /// This `MultipartWrite` takes a book that was just completed and then
    /// translates it into French, returning the translation as the final
    /// complete output.  This translation is a computation that happens in a
    /// future.
    ///
    /// Here it's mocked--the "translation" just replaces every line with a
    /// constant--but it could be the result of using a `reqwest::Client` to
    /// query `translate.googleapis.com/translate_a` with a request to translate
    /// the text of the book from English to a target language, for instance.
    fn into_french(
        self,
    ) -> impl MultipartWrite<String, Ret = BookState, Output = Book, Error = String> {
        self.and_then(Translator::get_translation)
    }
}

/// The story to write.
#[derive(Debug, Clone, Copy)]
struct Narrative;

impl Narrative {
    fn stream(lines: usize) -> impl Stream<Item = String> {
        stream::iter(Self).take(lines)
    }
}

impl Iterator for Narrative {
    type Item = String;
    fn next(&mut self) -> Option<Self::Item> {
        Some("All work and no play make Jack a dull boy.".into())
    }
}

/// A line is some text.
#[derive(Debug, Clone)]
struct Line(String);

impl Display for Line {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
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
    fn write_line(&mut self, line_no: usize, text: String) -> Result<usize, String> {
        let line = format!("{line_no:03} | {text}");
        self.0.push(Line(line));
        Ok(self.0.len())
    }

    fn sort(&mut self) {
        self.0.sort_by(|p1, p2| p1.0.cmp(&p2.0));
    }
}

/// Ordering of the `Page`s in a book.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Ord, PartialOrd)]
struct PageNumber(usize);

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

/// A `Book` has an edition and a mapping of page number to page.
#[derive(Debug, Clone)]
struct Book(usize, BTreeMap<PageNumber, Page>);

impl Default for Book {
    fn default() -> Self {
        Self(1, BTreeMap::default())
    }
}

impl Book {
    /// Return the string representation of the book's edition.
    fn edition(&self) -> String {
        format!("Ed. {}", self.0)
    }

    /// Start the next book by incrementing the edition and starting a new map of
    /// page number to page.
    fn start_next(&self) -> Book {
        Book(self.0 + 1, BTreeMap::default())
    }

    /// Write a page to the book.
    ///
    /// This returns an error if the book already has a page `page_number`.
    fn write_page(&mut self, page_number: PageNumber, mut page: Page) -> Result<(), String> {
        if self.1.contains_key(&page_number) {
            return Err(format!("page {} already exists", page_number.0));
        }
        page.sort();
        self.1.insert(page_number, page);
        Ok(())
    }

    fn iter_mut(&mut self) -> impl Iterator<Item = (&PageNumber, &mut Page)> {
        self.1.iter_mut()
    }
}

impl Display for Book {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let edition = self.edition();
        if self.1.is_empty() {
            return Ok(());
        }
        self.1.iter().try_for_each(|(page_number, page)| {
            let mut len = 0;
            let pg = format!("[{}]", page_number.0);
            writeln!(f, "The Book, {edition}")?;
            for line in &page.0 {
                writeln!(f, "{}", line)?;
                len = line.0.len() + 5;
            }
            writeln!(f, "{pg:^len$} ")?;
            Ok(())
        })
    }
}

/// `Author` is the example of a typical multipart writer.
///
/// We repeatedly write `Line`s to a `Page`.  The `Page` has a configured line
/// limit.  When the limit is achieved, we `poll_flush` to finish the page and
/// add it to the `Book`, starting a new page.  When a line is written, the
/// current page number and size is returned to the caller in case they want to
/// flush or complete the writer based on that information.
///
/// When enough pages have been written, `poll_complete` finishes the `Book`.
struct Author {
    book: Book,
    page: Page,
    line_limit: usize,
    current_page: PageNumber,
    current_line: usize,
}

impl Default for Author {
    fn default() -> Self {
        Self::new(10)
    }
}

impl Author {
    fn new(line_limit: usize) -> Self {
        Self {
            book: Book::default(),
            page: Page::new(line_limit),
            line_limit,
            current_page: PageNumber::default(),
            current_line: 1,
        }
    }

    /// Finish a page, returning the page number of the new page.
    ///
    /// This returns an error if there was already a page with the current page
    /// number.
    fn new_page(&mut self) -> Result<(), String> {
        if self.page.line_count() == 0 {
            return Ok(());
        }
        let finished_page = std::mem::replace(&mut self.page, Page::new(self.line_limit));
        self.book.write_page(self.current_page, finished_page)?;
        self.current_page.new_page();
        self.current_line = 1;
        Ok(())
    }

    fn book_state(&self, lines_written: usize) -> BookState {
        BookState {
            page_number: self.current_page,
            lines_written,
        }
    }
}

/// The current progress of the book is the return type of this writer.
#[derive(Debug, Clone, Copy)]
struct BookState {
    page_number: PageNumber,
    lines_written: usize,
}

impl BookState {
    fn page_number(&self) -> usize {
        self.page_number.0
    }
}

impl Display for BookState {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "current page: {}, total lines: {}",
            self.page_number.0, self.lines_written
        )
    }
}

impl MultipartWrite<String> for Author {
    type Ret = BookState;
    type Output = Book;
    type Error = String;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // This method needs to return `Poll::Pending` until there are less than
        // `line_limit` lines.  Flushing would return `Poll::Pending` until there
        // are 0 lines, so this is an appropriate way to implement `poll_ready`.
        if self.page.line_count() >= self.line_limit {
            ready!(self.poll_flush(cx))?;
        }
        Poll::Ready(Ok(()))
    }

    fn start_send(mut self: Pin<&mut Self>, text: String) -> Result<Self::Ret, Self::Error> {
        // Start the process of writing a line.
        let line_no = self.current_line;
        let lines_written = self.page.write_line(line_no, text)?;
        self.current_line += 1;
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
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Output, Self::Error>> {
        // The contract of `poll_complete` is fulfilled in this writer's case if
        // all pages are written.  So the pending page `self.page` needs to be
        // finished no matter how many lines it has, just as long as it has at
        // least one.
        if self.page.line_count() > 0 {
            ready!(self.as_mut().poll_flush(cx))?;
        }
        let new_book = self.book.start_next();
        let book = std::mem::replace(&mut self.book, new_book);
        self.current_page = PageNumber::default();
        self.current_line = 1;
        Poll::Ready(Ok(book))
    }
}

/// `Translator` becomes a writer when combined with an `Author`, or any writer
/// whose output type is `Book`.  This writer chains an asynchronous computation
/// on the output of the inner writer.
///
/// This const is used to mock the asynchronous computation that outputs a French
/// translation.
const LINE_FR: &str = "Tout le travail et aucun jeu font de Jack un garçon ennuyeux.";

#[derive(Debug, Clone, Copy, Default)]
struct Translator;

impl Translator {
    /// Mocking a call to  API to translate or something.
    fn get_translation(mut book: Book) -> impl Future<Output = Result<Book, String>> {
        book.iter_mut().for_each(|(_, pg)| Self::translate_page(pg));
        future::ready(Ok(book))
    }

    fn translate_page(pg: &mut Page) {
        let new_lines = std::iter::repeat_n(LINE_FR.to_string(), pg.line_count())
            .enumerate()
            .map(|(n, txt)| Line(format!("{n:03} | {txt}")))
            .collect();
        pg.0 = new_lines;
    }
}
