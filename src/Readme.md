# htmliter

Provides an streaming iterator api for iterating over elements (and their context path) in html or xml documents. This can be used to perform transformations or extractions on documents without loading them into memory.

The iterated item is a reference to a trait object which allows borrowing an xml/html item (element, text, cdata, ...) and all of it's ancestor elements.

The iterator chaining methods on `std::iter::Iterator` and the collectors which implement [`std::iter::FromIterator`] wouldn't be generally useful for iterating over these paths, so here we provide some iterator chaining methods and collectors, these would allow terse solutions for simple cases and pre-processing, but in more complex cases you are likely to be need to use `while let Some(item) = iter.next() {...}`.

## Examples

For simple examples you can use the chaining methods, but they obfuscate how a more complex problem would be solved, so both forms are shown here.

### Remove everything except the main element

```rust
# let read = BufReader::new(Cursor::new(""));
write!(&mut write, "{}", HtmlIter::from_reader(read).include(css_select!("main")));
```

```rust
# let read = BufReader::new(Cursor::new(""));
let mut iter = HtmlIter::from_reader(read);
while let Some(item) = iter.next() {
    if let Some(under_main) = item.include(css_select!("main")) { // anything without main in the path is ignored, and any context with main is stripped before the main
        write!(&mut write, "{}", under_main)?
    }
}
# Ok::<(), io::Error>(())
```

### Remove everything with the class "bloat" under "main"

```rust
# let read = BufReader::new(Cursor::new(""));
write!(&mut write, "{}", HtmlIter::from_reader(read).exclude(css_select!(("main") (."bloat"))));
```

```rust
# let read = BufReader::new(Cursor::new(""));
let mut iter = HtmlIter::from_reader(read);
while let Some(item) = iter.next() {
    if !item.match_any(css_select!(("main") (."bloat"))) { // if nothing in the item's path matches
        write!(&mut write, "{}", item)?
    }
}
```

### Remove all the `id`s from the document

```rust
write!(&mut write, "{}", HtmlIter::from_reader(read).map_all(|element| // on the iterator, `map_all` means that this mapping applies to every element in the document
    element.map_attributes(|attributes| // to map the element, we map the attributes, meaning no allocations need to take place, and any elements later ignored don't actually need to be processed
        attributes.filter(|name, _value|name != "id")))); // the attributes are a regular `std::iter::Iterator`
```

```rust
# let read = BufReader::new(Cursor::new(""));
let mut iter = HtmlIter::from_reader(read);
while let Some(item) = iter.next() {
    let item = item.map_all(|element| // on the item, `map_all` means this mapping applies to every element in the path
        element.map_attributes(|attributes|
            attributes.filter(|name, _value|name != "id")));
    write!(&mut write, "{}", item)?
}
```

### Extract all hyperlinks

```rust
let links: Vec<_> = HtmlIter::from_reader(read).filter_map(|item| (item.name == "a").then(|| item.attr("href"))).collect(); // methods on the iterator which are identically named to those on `std::iter::Iterator` work in the expected way, returning an `std::iter::Iterator`.
```

### Extract books expressed in RDFa

Here is a more complex example combining both chaining transformers and an imperative loop. The construction of a `Book` involves storing selected bits of data while looping and, when complete constructing a complete object if possible. This is best achieved with an imperative loop and mutable state.

```rust
# let read = BufReader::new(Cursor::new(""));
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

```
div#main .bloat => ("div" #"main") (."bloat)
```
