use std::mem;

use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};

mod element_path;

pub use element_path::*;

pub(crate) struct Traverser {
    buf: Vec<u8>,
    path: ElementPathBuf,
    drop_last: bool,
    current: Option<Node>,
}

impl Traverser {
    pub(crate) fn new() -> Traverser {
        Self {
            buf: vec![],
            path: ElementPathBuf::new(),
            drop_last: false,
            current: None,
        }
    }

    pub(crate) fn read_from<B: std::io::BufRead>(&mut self, reader: &mut quick_xml::Reader<B>) {
        if self.drop_last {
            self.path.path.pop().unwrap();
            self.drop_last = false;
        }
        self.current = match reader.read_event(&mut self.buf) {
            Ok(e) => match e {
                Event::Start(start) => Some(self.path.start(start, reader)),
                Event::End(end) => {
                    let decode = reader.decode(end.name()).unwrap();
                    let element = self.path.path.last().unwrap();
                    self.drop_last = true;
                    assert_eq!(decode, element.name);
                    Some(self.path.end())
                }
                Event::Empty(_) => todo!(),
                Event::Text(text) => {
                    Some(self.path.text(text.unescape_and_decode(reader).unwrap()))
                }
                Event::Comment(_) => todo!(),
                Event::CData(_) => todo!(),
                Event::Decl(_) => todo!(),
                Event::PI(_) => todo!(),
                Event::DocType(text) => {
                    Some(self.path.doctype(text.unescape_and_decode(reader).unwrap()))
                }
                Event::Eof => None,
            },
            Err(e) => panic!("{}", e),
        }
    }

    pub fn get(&self) -> Option<RawItem> {
        self.current.as_ref().map(|node| RawItem {
            context: self.path.as_path(),
            node: node.clone(),
        })
    }
}

impl<'a> Item<'a> for RawItem<'a> {
    fn as_element(&self) -> Option<RawElement<'a>> {
        match self.node {
            Node::Start | Node::End => self
                .context
                .path
                .last()
                .map(|en| self.context.as_element(en)),
            _ => None,
        }
    }

    /// The element path, including the element itself if it is one
    fn as_path(&self) -> ElementPath<'a> {
        self.context
    }

    fn node(&self) -> &Node {
        &self.node
    }
}

pub trait Item<'a> {
    fn node(&self) -> &Node;

    fn as_element(&self) -> Option<RawElement<'a>>;

    // /// The element path, not including the potential current element
    // pub(crate) fn into_context_path(self) -> ElementPath<'a>;

    // /// The element path, not including the potential current element
    // pub(crate) fn context_path(&self) -> ElementPath<'_>;

    /// The element path, including the element itself if it is one
    fn as_path(&self) -> ElementPath<'a>;

    fn as_event(&self) -> Event<'static> {
        use std::fmt::Write;

        match self.node() {
            Node::Text(ref unescaped) => {
                let bytes_text = BytesText::from_escaped_str(unescaped).into_owned();
                Event::Text(bytes_text)
            }
            Node::DocType(ref text) => {
                Event::DocType(BytesText::from_escaped_str(text).into_owned())
            }
            Node::Start => {
                let element = self.as_path().path.last().unwrap();
                let mut s = element.name.clone();
                let name_len = s.len();
                for NormalisedAttribute { name, value } in &element.attrs {
                    write!(&mut s, r#" {}="{}""#, name, value).unwrap();
                }
                Event::Start(BytesStart::owned(s, name_len))
            }
            Node::End => Event::End(BytesEnd::owned(
                self.as_path()
                    .path
                    .last()
                    .unwrap()
                    .name
                    .clone()
                    .into_bytes(),
            )),
        }
    }

    /// The element path, not including the potential current element
    fn context_path(&self) -> ElementPath<'a> {
        match self.node() {
            Node::Start | Node::End => {
                let path = self.as_path();
                path.slice(0..(path.path.len() - 1))
            }
            _ => self.as_path(),
        }
    }

    fn map_all<F, E1, E2>(self, map: F) -> MappedItem<Self, F>
    where
        E1: Element,
        E2: Element,
        F: Fn(&E1) -> E2,
        Self: Sized,
    {
        MappedItem {
            inner: self,
            _map: map,
        }
    }
}

pub struct MappedItem<I, F> {
    _map: F,
    inner: I,
}

impl<'a, I, F> Item<'a> for MappedItem<I, F>
where
    I: Item<'a>,
    F: Fn(RawElement) -> RawElement,
{
    fn node(&self) -> &Node {
        self.inner.node()
    }

    fn as_element(&self) -> Option<RawElement<'a>> {
        todo!("Element also needs to be a trait")
    }

    fn as_path(&self) -> ElementPath<'a> {
        todo!("ElementPath should be a trait")
    }
}

pub trait Element {
    fn name(&self) -> &str;

    fn attr(&self, search: &str) -> Option<&str>;

    fn classes(&self) -> Classes {
        match self.attr("class") {
            Some(s) => Classes { s },
            None => Classes { s: "" },
        }
    }

    fn filter_attributes<F>(&self, predicate: F) -> FilterAttributes<Self, F>
    where
        F: Fn(&str, &str) -> bool,
    {
        FilterAttributes {
            inner: self,
            predicate,
        }
    }
}

impl<'a> Element for RawElement<'a> {
    fn name(&self) -> &str {
        &self.element.name
    }

    fn attr(&self, search: &str) -> Option<&str> {
        for NormalisedAttribute { name, value } in self.attributes() {
            if name == search {
                return Some(value);
            }
        }
        None
    }
}

pub struct Classes<'a> {
    s: &'a str,
}

impl<'a> Iterator for Classes<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some((c, rest)) = self.s.split_once(' ') {
            self.s = rest;
            Some(c)
        } else if !self.s.is_empty() {
            Some(mem::take(&mut self.s))
        } else {
            None
        }
    }
}

pub struct FilterAttributes<'i, I: ?Sized, P: Fn(&str, &str) -> bool> {
    inner: &'i I,
    predicate: P,
}

impl<'i, I, P> Element for FilterAttributes<'i, I, P>
where
    I: Element + ?Sized,
    P: Fn(&str, &str) -> bool,
{
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn attr(&self, search: &str) -> Option<&str> {
        self.inner
            .attr(search)
            .and_then(|value| (self.predicate)(search, value).then_some(value))
    }
}
