use std::{borrow::Cow, io::Write};

use html5ever::{
    serialize::{self, Serializer},
    Attribute,
};

mod traverser;

pub use traverser::*;

pub struct HtmlPathElement<'a, Handle> {
    handle: Handle,
    name: html5ever::QualName,
    attrs: Cow<'a, [Attribute]>,
}

pub type HtmlPath<'a, Handle> = &'a [HtmlPathElement<'a, Handle>];

pub trait HtmlSink<InputHandle>
where
    InputHandle: Eq + Copy,
{
    fn append_doctype_to_document(
        &mut self,
        name: html5ever::tendril::StrTendril,
        public_id: html5ever::tendril::StrTendril,
        system_id: html5ever::tendril::StrTendril,
    );

    fn append_element(
        &mut self,
        path: HtmlPath<'_, InputHandle>,
        element: HtmlPathElement<'_, InputHandle>,
    );

    fn append_text(&mut self, path: HtmlPath<InputHandle>, text: &str);

    fn finish(self);
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

impl<Wr: Write, InputHandle: Eq + Copy> HtmlSink<InputHandle> for HtmlSerializer<Wr, InputHandle> {
    fn append_element(
        &mut self,
        path: HtmlPath<'_, InputHandle>,
        element: HtmlPathElement<'_, InputHandle>,
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
            name: element.name,
        });
    }

    fn append_text(&mut self, path: HtmlPath<InputHandle>, text: &str) {
        self.pop_to_path(path);

        self.inner.write_text(text).unwrap();
    }

    fn finish(mut self) {
        self.pop_to_path(&[])
    }

    fn append_doctype_to_document(
        &mut self,
        name: html5ever::tendril::StrTendril,
        _public_id: html5ever::tendril::StrTendril,
        _system_id: html5ever::tendril::StrTendril,
    ) {
        self.inner.write_doctype(&name).unwrap()
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
    fn append_doctype_to_document(
        &mut self,
        name: html5ever::tendril::StrTendril,
        public_id: html5ever::tendril::StrTendril,
        system_id: html5ever::tendril::StrTendril,
    ) {
        self.inner
            .append_doctype_to_document(name, public_id, system_id)
    }

    fn append_element(
        &mut self,
        path: HtmlPath<'_, InputHandle>,
        element: HtmlPathElement<'_, InputHandle>,
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

    fn finish(self) {
        self.inner.finish()
    }
}

pub struct ElementSelector<InputHandle: Eq + Copy, S: HtmlSink<InputHandle>> {
    inner: S,
    tags: Vec<&'static str>,
    select_handle: Option<InputHandle>,
}

impl<InputHandle: Eq + Copy, S: HtmlSink<InputHandle>> ElementSelector<InputHandle, S> {
    pub fn wrap(sink: S) -> Self {
        Self {
            inner: sink,
            tags: vec![],
            select_handle: None,
        }
    }

    pub fn tag(mut self, tag: &'static str) -> Self {
        self.tags.push(tag);
        Self {
            inner: self.inner,
            tags: self.tags,
            select_handle: self.select_handle,
        }
    }
}

impl<InputHandle: Eq + Copy, S: HtmlSink<InputHandle>> HtmlSink<InputHandle>
    for ElementSelector<InputHandle, S>
{
    fn append_doctype_to_document(
        &mut self,
        _name: html5ever::tendril::StrTendril,
        _public_id: html5ever::tendril::StrTendril,
        _system_id: html5ever::tendril::StrTendril,
    ) {
    }

    fn append_element(
        &mut self,
        path: HtmlPath<'_, InputHandle>,
        element: HtmlPathElement<'_, InputHandle>,
    ) {
        if let Some(select_handle) = self.select_handle {
            if let Some(select_index) = path
                .iter()
                .enumerate()
                .find_map(|(index, elem)| (elem.handle == select_handle).then(|| index))
            {
                self.inner.append_element(&path[select_index..], element)
            } else {
                self.select_handle = None
            }
        } else {
            let select = self.tags.contains(&&*element.name.local);
            if select {
                self.select_handle = Some(element.handle);
                self.inner.append_element(&[], element)
            }
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

    fn finish(self) {
        self.inner.finish()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use html5ever::{
        local_name, namespace_url, ns, serialize::SerializeOpts, tendril::TendrilSink, ParseOpts,
        QualName,
    };

    #[test]
    fn doc_identity() {
        let mut buf = Vec::new();
        let sink = HtmlSerializer::new(&mut buf, SerializeOpts::default());
        let parser = parse_document(sink, ParseOpts::default());
        let test = "<!DOCTYPE html><html><head></head><body><p><b>hello</b></p><p>world!</p></body></html>";
        parser.one(test);
        assert_eq!(String::from_utf8(buf).unwrap(), test);
    }

    #[ignore = "haven't figured out how to do fragments yet"]
    fn fragment_identity() {
        let mut buf = Vec::new();
        let sink = HtmlSerializer::new(&mut buf, SerializeOpts::default());
        let parser = parse_fragment(
            sink,
            ParseOpts::default(),
            QualName {
                prefix: None,
                ns: ns!(),
                local: local_name!("body"),
            },
            vec![],
        );
        let test = "<p><b>hello</b></p><p>world!</p>";
        parser.one(test);
        assert_eq!(String::from_utf8(buf).unwrap(), test);
    }

    #[test]
    fn remove_elements() {
        let mut buf = Vec::new();
        let sink = ElementRemover::wrap(HtmlSerializer::new(&mut buf, SerializeOpts::default()))
            .class("hello");
        let parser = parse_document(sink, ParseOpts::default());
        let test = r#"<!DOCTYPE html><html><head></head><body><p class="hello"><b>hello</b></p><p>world!</p></body></html>"#;
        parser.one(test);
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            r#"<!DOCTYPE html><html><head></head><body><p>world!</p></body></html>"#
        );
    }

    #[test] // for selection, a selected node needs to be appended to the document, if it is not already part of a selected tree. i think for this all to work, either each processor needs to have it's own traversal tree, or maybe, the traversal tree builder from a Sink is only the first step and the processing actually happens using a different interface, probably entirely triggered by appends, but also having a (filtered) access to the tracversal scope
    fn select_element() {
        let mut buf = Vec::new();
        let sink = ElementSelector::wrap(HtmlSerializer::new(&mut buf, SerializeOpts::default()))
            .tag("body");
        let parser = parse_document(sink, ParseOpts::default());
        let test = "<!DOCTYPE html><html><head></head><body><p><b>hello</b></p><p>world!</p></body></html>";
        parser.one(test);
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "<body><p><b>hello</b></p><p>world!</p></body>"
        );
    }

    #[test]
    fn extract_data() {}
}
