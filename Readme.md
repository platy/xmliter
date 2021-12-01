# html 5 streams toolkit

(Under development) toolkit for stream processing html documents.

The kinds of processing I'm interested in implementing here are things like:

* Removing elements (and optionally their children)
* Inserting elements (and possibly wrapping the children of an existing element)
* Marshalling into structs (like strong-xml)
* Structural diffing

The streaming requirements:

* Memory usage is proportional to the depth of the html tree, and not the document length or number of elements
* Allocations should be very minimal, the worst case could be allocating proportionally to the number of elements in a document
* The operations above can be chained so they need to have the same interface at input and output
* A stream can be forked to produce multiple outputs, possibly in parallel
* A serialisation of the output can only produce valid HTML, even if a filter is implemented badly
* Push-based: this is how html5ever works, the parser does all the calling, the serialiser receives calls. This is convenient for a fork as it can just call one and then the other (or perhaps in parallel) but the diff filter would require 2 sources on 2 threads and would then require some complex synchronisation between them.

## Sink trait

```rust
trait HtmlSink<InputHandle>
where InputHandle: Eq + Copy {

    fn append_element(&mut self, path: HtmlPath<'_, Self::InputHandle>);

    fn append_text(&mut self, path: HtmlPath<Self::InputHandle>, text: &str);

    /// fn append_other(...)
}

struct HtmlPathElement<'a, Handle> {
    handle: Handle,
    name: QualifiedName,
    attrs: Cow<'a, Vec<Attribute>>,
}

type HtmlPath<'a, Handle> = &[HtmlPathElement<'a, Handle>];
```


## Removing elements

Whether to remove can be decided based on the html path, then the filter can just record the caller's handle for the element that's being removed and either remove all children by ignoring every append which has that handle in the path, or keep the children by just filtering it out of the path.

## Inserting elements

The inserting filter can insert an element in the html path, it will need to generate it's own handle which means it's produced paths are `Either<OwnHandle, CallerHandle>`


## Diff filter

It's going to be diffing 2 different streams from 2 different threads, so maybe it will have it's state in a Mutex, when data comes in, it will lock the mutex, then update the state with it's element, if the algorithm isn't ready to write the input node, it will need to wait on the other thread. The 2 threads will therefore be in lockstep. A diff algorithm probably needs some amount of look-ahead, i'm not sure whether this can be done with limited memory or allocations.

## Further work

* Consider a different parser, html5ever is doing a lot of things to be ideal to building a dom, but since I'm not building a dom it's probably sub-optimal. The main thing is the internalised strings, there is no need to do this as the number of strings I actually need at any time is pretty low, and allocations could be kept low by reusing strings and vecs of elements which have gone out of scope.
* Consider switching to pull. Pulling instead of pushing might make chaining of transformations more ergonomic, it might also mean we can use a streaming iterator api. Pull would be much better for diff which wouldn't then need to synchronise 2 threads, it would just be one thread reading as needed from 2 iterators. On the other hand it makes it more difficult to split and produce 2 different collections from one source. But maybe that is easier to synchronise, if it was 2 threads building collections from one iterator, the first one to read just needs to leave the item borrowed for the second to use. Without a second thread stuff could also be collected (ie. using `Iterator::inspect`) but it wouldn't be able to use the same transformations and `FromIterator` for example.
* To switch to an iterator and transformations, I need a single trait for the whole path including the final node and probably doctype etc too. Then a transformation will map to something which implements the same type. So it should be able to be totally lazy. Actually, if I was collecting more than one thing from a single iteration in rust, I wouldn't do it with transformations, I would do it imperatively with a for loop, some if's and matches.

# htmliter

Provides an iterator api for iterating over elements (and their context path) in html or xml documents. This can be used to perform transformations or extractions on documents without loading them into memory.

The iterated item is a reference to a trait object which allows borrowing an xml/html item (element, text, cdata, ...) and all of it's ancestor elements. As the item is borrowed, it can't currently implement [`std::iter::Iterator`], but if borrow support is added to `Iterator` in the future, that will be added here and this could be used with for loops.

The iterator chaining methods on `std::iter::Iterator` and the collectors which implement [`std::iter::FromIterator`] wouldn't be generally useful for iterating over these paths, so here we provide some iterator chaining methods and collectors, these would allow terse solutions for simple cases and pre-processing, but in more complex cases you are likely to be need to use `while let Some(item) = iter.next() {...}`.

## Examples

For simple examples you can use the chaining methods, but they obfuscate how a more complex problem would be solved, so both forms are shown here.

### Remove everything except the main element

```rust
write!(&mut write, "{}", HtmlIter::from_reader(read).include(css_select!("main")));
```

```rust
let mut iter = HtmlIter::from_reader(read);
while let Some(item) = iter.next() {
    if let Some(under_main) = item.include(css_select!("main")) { // anything without main in the path is ignored, and any context with main is stripped before the main
        write!(&mut write, "{}", under_main)?
    }
}
```

### Remove everything with the class "bloat" under "main"

```rust
write!(&mut write, "{}", HtmlIter::from_reader(read).exclude(css_select!(("main") (."bloat"))));
```

```rust
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
let mut iter = HtmlIter::from_reader(read);
while let Some(item) = iter.next() {
    let item = item.map_all(|element| // on the item, `map_all` means this mapping applies to every element in the path
        element.map_attributes(|attributes|
            attributes.filter(|name, _value|name != "id"))));
    write!(&mut write, "{}", item)?
}
```

### Extract all hyperlinks

```rust
let links: Vec<_> = HtmlIter::from_reader(read).filter_map(|item| (item.name == "a").then(|| item.attr("href"))).collect(); // methods on the iterator which are identically named to those on `std::iter::Iterator` work in the expected way.
```

### Extract books expressed in RDFa

Here is a more complex example combining both chaining transformers and an imperative loop. The construction of a `Book` involves storing selected bits of data while looping and, when complete constructing a complete object if possible. This is best achieved with an imperative loop and mutable state.

```rust
struct Book {
    name: String,
    description: Option<String>,
    author: Option<String>
}

let books = HtmlIter::from_reader(read)
    // `group_under` gives us an item at this level for each mach in the document
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
                    Some("name") => name.extend(text),
                    Some("description") => desription.extend(text),
                    Some("author") => author.extend(text),
                    None => {}
                }
            }
        }
        // book is complete, see if we have enough data collected
        if !name.is_empty() {
            Some(Book {
                name,
                description: !description.is_empty().then(|| description),
                author: !author.is_empty().then(|| author),
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
