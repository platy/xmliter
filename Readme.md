# htmliter

Provides an streaming iterator api for iterating over elements (and their context path) in html or xml documents. This can be used to perform transformations or extractions on documents without loading them into memory.

The iterated item is a reference which allows borrowing an xml/html item (element, text, cdata, ...) and all of it's ancestor elements.

The iterator chaining methods on `std::iter::Iterator` and the collectors which implement [`std::iter::FromIterator`] wouldn't be generally useful for iterating over these paths, so here we provide some iterator chaining methods and collectors, these would allow terse solutions for simple cases and pre-processing, but in more complex cases you are likely to be need to use `while let Some(item) = iter.next() {...}`.

The streaming requirements:

* Memory usage is proportional to the depth of the html tree, and not the document length or number of elements
* Allocations should be very minimal, the worst case could be allocating proportionally to the number of elements in a document
* The operations above can be chained so they need to have the same interface at input and output
* A stream can be forked to produce multiple outputs, possibly in parallel
* A serialisation of the output can only produce valid HTML, even if a filter is implemented badly
* Push-based: this is how html5ever works, the parser does all the calling, the serialiser receives calls. This is convenient for a fork as it can just call one and then the other (or perhaps in parallel) but the diff filter would require 2 sources on 2 threads and would then require some complex synchronisation between them.

## Todo

- [ ] Reorganise modules - reduce the implementation complexity
- [ ] Tidy API surface (take cues from lending iterator) & document
- [ ] Reduce allocations
- [ ] Maybe shouldn't rely on whole `lending-terator` crate just for it's HKTs
- [ ] Maybe this can implement `lending-iterator`
- [ ] Inserting elements (and possibly wrapping the children of an existing element)
- [ ] Marshalling into structs (like strong-xml)
- [ ] Structural diffing
