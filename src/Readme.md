# htmliter

Provides an streaming iterator api for iterating over elements (and their context path) in html or xml documents. This can be used to perform transformations or extractions on documents without loading them into memory.

The iterated item is a reference to a trait object which allows borrowing an xml/html item (element, text, cdata, ...) and all of it's ancestor elements.

The iterator chaining methods on `std::iter::Iterator` and the collectors which implement [`std::iter::FromIterator`] wouldn't be generally useful for iterating over these paths, so here we provide some iterator chaining methods and collectors, these would allow terse solutions for simple cases and pre-processing, but in more complex cases you are likely to be need to use `while let Some(item) = iter.next() {...}`.

## Examples

For simple examples you can use the chaining methods, but they obfuscate how a more complex problem would be solved, so both forms are shown here.

### Remove everything except the main element

```rust
# use xmliter::*;
    let read = std::io::Cursor::new(
        "<!DOCTYPE html><html><body><main>content</main></body></html>",
    );
    let out = HtmlIter::from_reader(read)
        .include(css_select!("main"))
        .to_string();
    assert_eq!(out, "<main>content</main>");
```

```rust
# use xmliter::*;
    let read = std::io::Cursor::new(
        "<!DOCTYPE html><html><body><main>content</main></body></html>",
    );
    let mut iter = HtmlIter::from_reader(read);
    let mut out = Vec::new();
    let mut writer = HtmlWriter::from_writer(&mut out);
    while let Some(item) = iter.next() {
        if let Some(under_main) = item.include(&css_select!("main")) {
            // anything without main in the path is ignored, and any context with main is stripped before the main
            writer.write_item(&under_main);
        }
    }
    assert_eq!(String::from_utf8(out).unwrap(), "<main>content</main>");
```

### Remove everything with the class "bloat" under "main"

```rust
# use xmliter::*;
    let read = std::io::Cursor::new(
        r#"<!DOCTYPE html><html><body><main><p>content</p><p class="bloat">bloat</p></main></body></html>"#,
    );
    let out = HtmlIter::from_reader(read)
        .exclude(css_select!(("main") (."bloat")))
        .to_string();
    assert_eq!(
        out,
        "<!DOCTYPE html><html><body><main><p>content</p></main></body></html>"
    );
```

```rust
# use xmliter::*;
    let read = std::io::Cursor::new(
        r#"<!DOCTYPE html><html><body><main><p>content</p><p class="bloat">bloat</p></main></body></html>"#,
    );
    let mut iter = HtmlIter::from_reader(read);
    let mut out = Vec::new();
    let mut writer = HtmlWriter::from_writer(&mut out);
    while let Some(item) = iter.next() {
        if !css_select!(("main") (."bloat")).match_any(item.as_path()) {
            // if nothing in the item's path matches
            writer.write_item(&item);
        }
    }
    assert_eq!(
        String::from_utf8(out)?,
        "<!DOCTYPE html><html><body><main><p>content</p></main></body></html>"
    );
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Remove all the `id`s from the document

```rust
# use lending_iterator::HKT;
# use xmliter::*;
    let read = std::io::Cursor::new(
        r#"<!DOCTYPE html><html><body><main id="id"><p id="first">content</p><p class="bloat" id="second">bloat</p></main></body></html>"#,
    );
    let out = HtmlIter::from_reader(read)
        .map_all::<HKT!(FilterAttributes<RawElement<'_>, _>), _>(
            |_, element| // on the iterator, `map_all` means that this mapping applies to every element in the document
                element.filter_attributes(|name, _value|name != "id"),
        )
        .to_string(); // the attributes are a regular `std::iter::Iterator`
    assert_eq!(
        out,
        r#"<!DOCTYPE html><html><body><main><p>content</p><p class="bloat">bloat</p></main></body></html>"#
    );
```

```rust
# use xmliter::*;
    let read = std::io::Cursor::new(
        r#"<!DOCTYPE html><html><body><main id="id"><p id="first">content</p><p class="bloat" id="second">bloat</p></main></body></html>"#,
    );
    let mut iter = HtmlIter::from_reader(read);
    let mut out = Vec::new();
    let mut writer = HtmlWriter::from_writer(&mut out);
    while let Some(item) = iter.next() {
        let item = item.map_all(
            |_, element| // on the item, `map_all` means this mapping applies to every element in the path
            element.filter_attributes(|name, _value| name != "id"),
        );
        writer.write_item(&item);
    }
    assert_eq!(
        String::from_utf8(out).unwrap(),
        r#"<!DOCTYPE html><html><body><main><p>content</p><p class="bloat">bloat</p></main></body></html>"#
    );
```

### Extract all hyperlinks

```rust
# use xmliter::*;
let links: Vec<_> = HtmlIter::from_reader(read).filter_map(|item| (item.name == "a").then(|| item.attr("href"))).collect(); // methods on the iterator which are identically named to those on `std::iter::Iterator` work in the expected way, returning an `std::iter::Iterator`.
```

### Extract books expressed in RDFa

Here is a more complex example combining both chaining transformers and an imperative loop. The construction of a `Book` involves storing selected bits of data while looping and, when complete constructing a complete object if possible. This is best achieved with an imperative loop and mutable state.

```rust
# use xmliter::*;
# let read = std::io::Cursor::new("");
struct Book {
    name: String,
    description: Option<String>,
    author: Option<String>
}

let books = HtmlIter::from_reader(read)
    // `group_under` gives us an item at this level for each match in the document
    .group_under(css_select!((["vocab"="https://schema.org/"] ["typeof"="Book"])))
    // `filter_map` returns a regular iterator over `Book`
    .filter_map(|book| {
        let mut name = None;
        let mut description = None;
        let mut author = None;
        // this while loop happens for all nodes under each book, ancestors coming before the grouping element have been stripped
        while let Some(item) = book.next() {
            // if this is a text node
            if let Some(text) = item.text() {
                // the first ancestor with a property attribute
                match item.first_ancestor().attr("property") {
                    Some("name") => name = Some(text),
                    Some("description") => description = Some(text),
                    Some("author") => author = Some(text),
                    None => {}
                }
            }
        }
        // book is complete, see if we have enough data collected
        if let Some(name) = name {
            Some(Book {
                name,
                description,
                author,
            })
        } else {
            None
        }
    });
```

## CSS-like Selectors

The syntax of our selectors are inspired by CSS, to support CSS exactly we would need to use procedural macros, and I don't think this is a good enough reason to use them. The main changes are that identifiers are surround by strings (so that they can contain things not allowed in rust identifiers such as '-') and that groupings of selectors which are to match the same element are grouped with parens (because rust's tokeniser ignores whitespace). Mostly we're only supporting CSS level 1 - element names, ids, classes.

Let me know if you have any better ideas.

```ignore
div#main .bloat => ("div" #"main") (."bloat)
```
