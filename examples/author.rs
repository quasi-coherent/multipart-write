//! This example centers on an `Author`.
//!
//! The author writes books.  The books are created by writing lines to a page
//! until some configured limit, and pages to a book until some condition is
//! met.
//!
//! This is modeled as a `MultipartWrite` by having the part be a line on a
//! page, the return value of starting a write be the page number and current
//! line count, the flushing behavior be finishing a page and starting a new
//! page, and the completion of the writer be finishing the book.
use futures::{Future, Stream, StreamExt, future, ready, stream};
use multipart_write::stream::MultipartStreamExt as _;
use multipart_write::{
    BoxFusedMultipartWrite, BoxMultipartWrite, FusedMultipartWrite,
    MultipartWrite, MultipartWriteExt as _,
};
use std::collections::BTreeMap;
use std::fmt::{self, Display, Formatter};
use std::pin::Pin;
use std::task::{Context, Poll};

/// The story to write.
#[derive(Debug, Clone, Copy)]
struct Narrative;

impl Narrative {
    fn stream(lines: usize) -> impl Stream<Item = Line> {
        stream::iter(Self).map(Line).take(lines)
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
    fn write_line(&mut self, line_no: usize, line: Line) -> usize {
        let Line(text) = line;
        let with_ln = format!("{line_no:03} | {text}");
        self.0.push(Line(with_ln));
        self.0.len()
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
    /// Increment the page number the author is writing.
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
    /// Returns the string representation of the book's edition.
    fn edition(&self) -> String {
        format!("Ed. {}", self.0)
    }

    /// Start the next book by incrementing the edition and starting a new map
    /// of page number to page.
    fn start_next(&self) -> Book {
        Book(self.0 + 1, BTreeMap::default())
    }

    /// Write a page to the book.
    ///
    /// This returns an error if the book already has a page `page_number`.
    fn write_page(
        &mut self,
        page_number: PageNumber,
        mut page: Page,
    ) -> Result<(), String> {
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
                writeln!(f, "{line}")?;
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
///
/// _Remark_: We carry the type of part `L` in `Author<L>` to aid in type
/// inference, but this is not necessary.
struct Author<L> {
    book: Book,
    page: Page,
    line_limit: usize,
    current_page: PageNumber,
    current_line: usize,
    _line: std::marker::PhantomData<L>,
}

impl Default for Author<Line> {
    fn default() -> Self {
        Self::new(10)
    }
}

impl Author<Line> {
    fn new(line_limit: usize) -> Self {
        Self {
            book: Book::default(),
            page: Page::new(line_limit),
            line_limit,
            current_page: PageNumber::default(),
            current_line: 1,
            _line: std::marker::PhantomData,
        }
    }

    /// Finish a page, returning an error if there is a collision of page
    /// numbers.
    fn new_page(&mut self) -> Result<(), String> {
        if self.page.line_count() == 0 {
            return Ok(());
        }
        let finished_page =
            std::mem::replace(&mut self.page, Page::new(self.line_limit));
        self.book.write_page(self.current_page, finished_page)?;
        self.current_page.new_page();
        self.current_line = 1;
        Ok(())
    }

    fn book_state(&self, lines_written: usize) -> BookState {
        BookState { page_number: self.current_page, lines_written }
    }

    /// Used primarily for helping type inference.  Since `MultipartWrite` is
    /// generic over the part type, it is often not possible to infer various
    /// types that appear in use.
    fn boxed_author(
        self,
    ) -> BoxMultipartWrite<'static, Line, BookState, Book, String> {
        self.boxed()
    }

    /// This `MultipartWrite` augments the original implementation by reversing
    /// the input text before sending to the writer.  The result is a new writer
    /// that writes the same books but every line is backwards.
    fn reverse(self) -> BoxMultipartWrite<'static, Line, (), Book, String> {
        self.boxed_author()
            .ready_part(|line: Line| async move {
                let rev = line.0.chars().rev().collect();
                Ok(Line(rev))
            })
            .boxed()
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
    ) -> BoxFusedMultipartWrite<'static, Line, BookState, Book, String> {
        self.and_then(Translator::get_translation).box_fused()
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

// This is responsible for notifying a caller that it is not safe to poll and it
// never will be.
//
// In this case there is no concept of the internal state becoming invalid in
// any way, so this writer may be reused into perpetuity.
//
// We still go through the trouble of writing out `is_terminated` because being
// a fused writer is a requirement of select combinators, for example the stream
// `try_complete_when`.
impl FusedMultipartWrite<Line> for Author<Line> {
    fn is_terminated(&self) -> bool {
        false
    }
}

impl MultipartWrite<Line> for Author<Line> {
    // Some error type for this writer.
    type Error = String;
    // What is ultimately built from the parts.
    type Output = Book;
    // The type of value acknowledging receipt of a part.
    type Recv = BookState;

    // Has to be called prior to any `start_send`. It is what says that the
    // writer is prepared to accept more parts.
    fn poll_ready(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        // In this case, the method needs to return `Poll::Pending` until there
        // are less than `line_limit` lines.  Flushing returns pending until
        // there are 0 lines, so this is one way to write it correctly.
        if self.page.line_count() >= self.line_limit {
            ready!(self.poll_flush(cx))?;
        }
        Poll::Ready(Ok(()))
    }

    // Send the part (line) to the writer (author, who has a page in-progress).
    // What is returned represents that the part was received successfully, but
    // can be any type of value.  In this case it's the `BookState` tracking the
    // pages and lines written.
    //
    // In general, the type of value returned should be some information that
    // the caller might use to decide to flush or complete the writer.
    fn start_send(
        mut self: Pin<&mut Self>,
        line: Line,
    ) -> Result<Self::Recv, Self::Error> {
        // Start the process of writing a line.
        let line_no = self.current_line;
        let lines_written = self.page.write_line(line_no, line);
        self.current_line += 1;
        Ok(self.book_state(lines_written))
    }

    // Completely write any buffered parts, whatever that means.
    //
    // In this case we wrote `new_page` to internally update the state by adding
    // the current page to the book and starting a new one.
    fn poll_flush(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(self.new_page())
    }

    // Called when enough parts have been written.
    //
    // The method is meant to combine or assemble everything that's been written
    // into the associated `Self::Output`. For some types of writers, this
    // renders the value permanently unable to be written to further, but this
    // need not be the case.
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
/// This const is just used to mock the asynchronous computation that outputs a
/// French translation.
const LINE_FR: &str =
    "Tout le travail et aucun jeu font de Jack un garçon ennuyeux.";

#[derive(Debug, Clone, Copy, Default)]
struct Translator;

impl Translator {
    /// Mocking a call to some translator service, like Google Translate or
    /// something.
    fn get_translation(
        mut book: Book,
    ) -> impl Future<Output = Result<Book, String>> {
        book.iter_mut().for_each(|(_, pg)| Self::translate_page(pg));
        future::ready(Ok(book))
    }

    fn translate_page(pg: &mut Page) {
        let new_lines =
            std::iter::repeat_n(LINE_FR.to_string(), pg.line_count())
                .enumerate()
                .map(|(n, txt)| Line(format!("{n:03} | {txt}")))
                .collect();
        pg.0 = new_lines;
    }
}

const LIMIT: usize = 625;

/// Entrypoint of the example that prints the pages of the `Book` built by the
/// various `Author` types.
#[tokio::main]
async fn main() -> Result<(), String> {
    let short_story = Narrative::stream(LIMIT)
        .complete_with(Author::default())
        .await
        .unwrap();
    println!("{short_story}");
    println!("=========================");

    let short_story_reversed = Narrative::stream(LIMIT)
        .complete_with(Author::default().reverse())
        .await
        .unwrap();
    println!("{short_story_reversed}");
    println!("=========================");

    let books: Vec<Book> = Narrative::stream(LIMIT)
        .try_complete_when(Author::default(), |b| b.page_number() >= 25)
        .filter_map(|res| future::ready(res.ok()))
        .collect()
        .await;
    books.into_iter().for_each(|book| println!("{book}"));
    println!("=========================");

    let french_books: Vec<Book> = Narrative::stream(LIMIT)
        .try_complete_when(Author::default().into_french(), |b| {
            b.page_number() >= 25
        })
        .filter_map(|res| future::ready(res.ok()))
        .collect()
        .await;
    french_books.into_iter().for_each(|book| println!("{book}"));
    println!("=========================");

    Ok(())
}
