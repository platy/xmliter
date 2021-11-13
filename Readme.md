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

