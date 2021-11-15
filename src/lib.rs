use std::{borrow::Cow, io::Write, iter, marker::PhantomData, mem};

use html5ever::{
    serialize::{self, Serializer},
    Attribute,
};

mod traverser;

pub use traverser::*;

#[derive(Clone)]
pub struct HtmlPathElement<'a, Handle> {
    pub handle: Handle,
    pub name: html5ever::QualName,
    pub attrs: Cow<'a, [Attribute]>,
}

pub type HtmlPath<'a, Handle> = &'a [HtmlPathElement<'a, Handle>];

pub trait HtmlSink<InputHandle>: Sized
where
    InputHandle: Eq + Copy,
{
    type Output;

    fn append_doctype_to_document(
        &mut self,
        name: &html5ever::tendril::StrTendril,
        public_id: &html5ever::tendril::StrTendril,
        system_id: &html5ever::tendril::StrTendril,
    );

    fn append_element(
        &mut self,
        path: HtmlPath<'_, InputHandle>,
        element: &HtmlPathElement<'_, InputHandle>,
    );

    fn append_text(&mut self, path: HtmlPath<InputHandle>, text: &str);

    fn reset(&mut self) -> Self::Output;

    fn finish(mut self) -> Self::Output {
        self.reset()
    }
}

struct OpenElement<Handle> {
    handle: Handle,
    name: html5ever::QualName,
}

pub struct HtmlSerializer<Wr: Write, InputHandle> {
    inner: html5ever::serialize::HtmlSerializer<Wr>,
    open_element_path: Vec<OpenElement<InputHandle>>,
}

impl<Wr: Write, InputHandle: Eq> HtmlSerializer<Wr, InputHandle> {
    fn pop_to_path(&mut self, path: HtmlPath<'_, InputHandle>) {
        assert!(path.len() <= self.open_element_path.len());
        assert!(path
            .iter()
            .zip(&self.open_element_path)
            .all(|(a, b)| a.handle == b.handle));
        while path.len() < self.open_element_path.len() {
            let closed = self.open_element_path.pop().unwrap();
            self.inner.end_elem(closed.name).unwrap();
        }
    }

    pub fn new(writer: Wr, opts: serialize::SerializeOpts) -> Self {
        Self {
            inner: html5ever::serialize::HtmlSerializer::new(writer, opts),
            open_element_path: vec![],
        }
    }
}

impl<Wr: Write, InputHandle: Eq + Copy> HtmlSink<InputHandle>
    for &mut HtmlSerializer<Wr, InputHandle>
{
    type Output = ();

    fn append_element(
        &mut self,
        path: HtmlPath<'_, InputHandle>,
        element: &HtmlPathElement<'_, InputHandle>,
    ) {
        self.pop_to_path(path);

        self.inner
            .start_elem(
                element.name.clone(),
                element.attrs.iter().map(|att| (&att.name, &*att.value)),
            )
            .unwrap();
        self.open_element_path.push(OpenElement {
            handle: element.handle,
            name: element.name.clone(),
        });
    }

    fn append_text(&mut self, path: HtmlPath<InputHandle>, text: &str) {
        self.pop_to_path(path);

        self.inner.write_text(text).unwrap();
    }

    fn reset(&mut self) -> Self::Output {
        self.pop_to_path(&[])
    }

    fn append_doctype_to_document(
        &mut self,
        name: &html5ever::tendril::StrTendril,
        _public_id: &html5ever::tendril::StrTendril,
        _system_id: &html5ever::tendril::StrTendril,
    ) {
        self.inner.write_doctype(name).unwrap()
    }
}

pub struct ElementRemover<InputHandle: Eq + Copy, S: HtmlSink<InputHandle>> {
    inner: S,
    classes: Vec<&'static str>,
    skip_handle: Option<InputHandle>,
}

impl<InputHandle: Eq + Copy, S: HtmlSink<InputHandle>> ElementRemover<InputHandle, S> {
    pub fn wrap(sink: S) -> Self {
        Self {
            inner: sink,
            classes: vec![],
            skip_handle: None,
        }
    }

    pub fn class(mut self, class: &'static str) -> Self {
        self.classes.push(class);
        Self {
            inner: self.inner,
            classes: self.classes,
            skip_handle: self.skip_handle,
        }
    }
}

impl<InputHandle: Eq + Copy, S: HtmlSink<InputHandle>> HtmlSink<InputHandle>
    for ElementRemover<InputHandle, S>
{
    type Output = S::Output;

    fn append_doctype_to_document(
        &mut self,
        name: &html5ever::tendril::StrTendril,
        public_id: &html5ever::tendril::StrTendril,
        system_id: &html5ever::tendril::StrTendril,
    ) {
        self.inner
            .append_doctype_to_document(name, public_id, system_id)
    }

    fn append_element(
        &mut self,
        path: HtmlPath<'_, InputHandle>,
        element: &HtmlPathElement<'_, InputHandle>,
    ) {
        if let Some(skip_handle) = self.skip_handle {
            if path.iter().any(|elem| elem.handle == skip_handle) {
                return;
            } else {
                self.skip_handle = None
            }
        }
        let skip = element.attrs.iter().any(|a| {
            &a.name.local == "class"
                && a.value
                    .split_whitespace()
                    .any(|class| self.classes.contains(&class))
        });
        if skip {
            self.skip_handle = Some(element.handle);
            return;
        }
        self.inner.append_element(path, element)
    }

    fn append_text(&mut self, path: HtmlPath<InputHandle>, text: &str) {
        if let Some(skip_handle) = self.skip_handle {
            if path.iter().any(|elem| elem.handle == skip_handle) {
                return;
            } else {
                self.skip_handle = None
            }
        }
        self.inner.append_text(path, text)
    }

    fn reset(&mut self) -> Self::Output {
        self.skip_handle = None;
        self.inner.reset()
    }
}

pub struct ElementSelector<InputHandle: Eq + Copy, S: HtmlSink<InputHandle>, O = ()> {
    inner: S,
    tags: Vec<&'static str>,
    select_handle: Option<InputHandle>,
    output: O,
}

impl<InputHandle: Eq + Copy, S: HtmlSink<InputHandle>, O: Default>
    ElementSelector<InputHandle, S, O>
{
    pub fn wrap(inner: S) -> Self {
        Self {
            inner,
            tags: vec![],
            select_handle: None,
            output: O::default(),
        }
    }

    pub fn tag(mut self, tag: &'static str) -> Self {
        self.tags.push(tag);
        Self {
            inner: self.inner,
            tags: self.tags,
            select_handle: self.select_handle,
            output: self.output,
        }
    }
}

impl<InputHandle, S, O> HtmlSink<InputHandle> for ElementSelector<InputHandle, S, O>
where
    InputHandle: Eq + Copy,
    S: HtmlSink<InputHandle>,
    O: Extend<S::Output> + Default,
{
    type Output = O;

    fn append_doctype_to_document(
        &mut self,
        _name: &html5ever::tendril::StrTendril,
        _public_id: &html5ever::tendril::StrTendril,
        _system_id: &html5ever::tendril::StrTendril,
    ) {
    }

    fn append_element(
        &mut self,
        path: HtmlPath<'_, InputHandle>,
        element: &HtmlPathElement<'_, InputHandle>,
    ) {
        if let Some(select_handle) = self.select_handle {
            if let Some(select_index) = path
                .iter()
                .enumerate()
                .find_map(|(index, elem)| (elem.handle == select_handle).then(|| index))
            {
                // select continues
                self.inner.append_element(&path[select_index..], element);
                return;
            } else {
                // select ends
                self.select_handle = None;
                self.output.extend(iter::once(self.inner.reset()));
            }
        }
        let select = self.tags.contains(&&*element.name.local);
        if select {
            // select starts
            let select_handle = element.handle;
            self.inner.append_element(&[], element);
            self.select_handle = Some(select_handle);
        }
    }

    fn append_text(&mut self, path: HtmlPath<InputHandle>, text: &str) {
        if let Some(select_handle) = self.select_handle {
            if let Some(select_index) = path
                .iter()
                .enumerate()
                .find_map(|(index, elem)| (elem.handle == select_handle).then(|| index))
            {
                self.inner.append_text(&path[select_index..], text)
            } else {
                self.select_handle = None
            }
        }
    }

    fn reset(&mut self) -> Self::Output {
        if self.select_handle.take().is_some() {
            self.output.extend(iter::once(self.inner.reset()));
            self.select_handle = None
        }
        mem::take(&mut self.output)
    }
}

pub struct ElementSkipper<InputHandle: Eq + Copy, S: HtmlSink<InputHandle>> {
    inner: S,
    tags: Vec<&'static str>,
    _data: PhantomData<InputHandle>, // maybe better to make the handle into a trait associated type
}

impl<InputHandle: Eq + Copy, S: HtmlSink<InputHandle>> ElementSkipper<InputHandle, S> {
    pub fn wrap(inner: S) -> Self {
        Self {
            inner,
            tags: vec![],
            _data: PhantomData::default(),
        }
    }

    pub fn tag(mut self, tag: &'static str) -> Self {
        self.tags.push(tag);
        Self {
            inner: self.inner,
            tags: self.tags,
            _data: PhantomData::default(),
        }
    }
}

impl<InputHandle, S> HtmlSink<InputHandle> for ElementSkipper<InputHandle, S>
where
    InputHandle: Eq + Copy,
    S: HtmlSink<InputHandle>,
{
    type Output = S::Output;

    fn append_doctype_to_document(
        &mut self,
        _name: &html5ever::tendril::StrTendril,
        _public_id: &html5ever::tendril::StrTendril,
        _system_id: &html5ever::tendril::StrTendril,
    ) {
    }

    fn append_element(
        &mut self,
        path: HtmlPath<'_, InputHandle>,
        element: &HtmlPathElement<'_, InputHandle>,
    ) {
        if self.tags.contains(&&*element.name.local) {
            return;
        }
        // TODO optimise when not hitting
        let filtered_path = path
            .iter()
            .filter(|element| !self.tags.contains(&&*element.name.local))
            .cloned()
            .collect::<Vec<_>>();
        self.inner.append_element(filtered_path.as_slice(), element);
    }

    fn append_text(&mut self, path: HtmlPath<InputHandle>, text: &str) {
        // TODO optimise when not hitting
        let filtered_path = path
            .iter()
            .filter(|element| !self.tags.contains(&&*element.name.local))
            .cloned()
            .collect::<Vec<_>>();
        self.inner.append_text(filtered_path.as_slice(), text);
    }

    fn reset(&mut self) -> Self::Output {
        self.inner.reset()
    }
}

impl<Handle: Copy + Eq, A: HtmlSink<Handle>, B: HtmlSink<Handle>> HtmlSink<Handle> for (A, B) {
    type Output = (A::Output, B::Output);

    fn append_doctype_to_document(
        &mut self,
        name: &html5ever::tendril::StrTendril,
        public_id: &html5ever::tendril::StrTendril,
        system_id: &html5ever::tendril::StrTendril,
    ) {
        self.0
            .append_doctype_to_document(name, public_id, system_id);
        self.1
            .append_doctype_to_document(name, public_id, system_id);
    }

    fn append_element(
        &mut self,
        path: HtmlPath<'_, Handle>,
        element: &HtmlPathElement<'_, Handle>,
    ) {
        self.0.append_element(path, element);
        self.1.append_element(path, element);
    }

    fn append_text(&mut self, path: HtmlPath<Handle>, text: &str) {
        self.0.append_text(path, text);
        self.1.append_text(path, text);
    }

    fn reset(&mut self) -> Self::Output {
        (self.0.reset(), self.1.reset())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use html5ever::{
        serialize::{SerializeOpts, TraversalScope},
        tendril::TendrilSink,
        ParseOpts,
    };

    fn stream_doc(test: &str, sink: impl HtmlSink<u32>) {
        let mut opts = ParseOpts::default();
        opts.tree_builder.exact_errors = true;
        let parser = parse_document(sink, opts);
        parser.one(test);
    }

    fn serialiser(buf: &mut Vec<u8>) -> HtmlSerializer<&mut Vec<u8>, u32> {
        let opts = SerializeOpts::default();
        HtmlSerializer::new(buf, opts)
    }

    #[test]
    fn doc_identity() {
        let mut buf = Vec::new();
        let mut sink = serialiser(&mut buf);
        let test = "<!DOCTYPE html><html><head></head><body><p><b>hello</b></p><p>world!</p></body></html>";
        stream_doc(test, &mut sink);
        assert_eq!(String::from_utf8(buf).unwrap(), test);
    }

    #[test]
    // #[ignore = "html5ever mysteriously adds a <html> root"]
    fn fragment_identity() {
        let mut buf = Vec::new();
        let mut opts = SerializeOpts::default();
        opts.traversal_scope = TraversalScope::ChildrenOnly(None);
        let mut sink = HtmlSerializer::new(&mut buf, opts);
        let mut opts = ParseOpts::default();
        opts.tree_builder.exact_errors = true;
        let parser = parse_fragment(&mut sink, opts);
        let test = "<p><b>hello</b></p><p>world!</p>";
        parser.one(test);
        assert_eq!(String::from_utf8(buf).unwrap(), test);
    }

    #[test]
    fn remove_elements() {
        let mut buf = Vec::new();
        let mut serializer = serialiser(&mut buf);
        let test = r#"<!DOCTYPE html><html><head></head><body><p class="hello"><b>hello</b></p><p>world!</p></body></html>"#;
        stream_doc(test, ElementRemover::wrap(&mut serializer).class("hello"));
        assert_eq!(
            buf,
            br#"<!DOCTYPE html><html><head></head><body><p>world!</p></body></html>"#
        );
    }

    #[test] // for selection, a selected node needs to be appended to the document, if it is not already part of a selected tree. i think for this all to work, either each processor needs to have it's own traversal tree, or maybe, the traversal tree builder from a Sink is only the first step and the processing actually happens using a different interface, probably entirely triggered by appends, but also having a (filtered) access to the tracversal scope
    fn select_element() {
        let mut buf = Vec::new();
        let mut serializer = serialiser(&mut buf);
        let sink = ElementSelector::<_, _>::wrap(&mut serializer).tag("p");
        let test = "<!DOCTYPE html><html><head></head><body><p><b>hello</b></p><p>world!</p></body></html>";
        stream_doc(test, sink);
        assert_eq!(buf, b"<p><b>hello</b></p><p>world!</p>");
    }

    #[test]
    fn extract_data() {}
}
