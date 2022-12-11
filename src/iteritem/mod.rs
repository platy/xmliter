use std::marker::PhantomData;

use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};

mod element;
mod element_path;

use crate::ElementOfItem;
use crate::ElementOfPath;

pub use self::element::Element;
pub use self::element::ElementHasAttributes;
pub use self::element::FilterAttributes;
pub use self::element_path::*;

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
        self.current = match reader.read_event_into(&mut self.buf) {
            Ok(e) => match e {
                Event::Start(start) => Some(self.path.start(start, reader)),
                Event::End(end) => {
                    let element_name = end.name();
                    let decode = reader.decoder().decode(element_name.as_ref()).unwrap();
                    let element = self.path.path.last().unwrap();
                    self.drop_last = true;
                    assert_eq!(decode, element.name);
                    Some(self.path.end())
                }
                Event::Empty(_) => todo!(),
                Event::Text(text) => Some(self.path.text(text.unescape().unwrap().into_owned())),
                Event::Comment(_) => todo!(),
                Event::CData(_) => todo!(),
                Event::Decl(_) => todo!(),
                Event::PI(_) => todo!(),
                Event::DocType(text) => {
                    Some(self.path.doctype(text.unescape().unwrap().into_owned()))
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

pub trait Item<'a> {
    type Path: ElementPath<'a>;

    fn node(&self) -> &Node;

    fn as_element(&self) -> Option<<Self::Path as ElementPath<'a>>::E>;

    // /// The element path, not including the potential current element
    // pub(crate) fn into_context_path(self) -> ElementPath<'a>;

    // /// The element path, not including the potential current element
    // pub(crate) fn context_path(&self) -> ElementPath<'_>;

    /// The element path, including the element itself if it is one
    fn as_path(&self) -> Self::Path;

    /// As a quick-xml event, for serialisation, allocates for start tags but not for others
    fn as_event<'b>(&'b self) -> Event<'b>
    where
        Self: 'b,
        'a: 'b,
        <Self::Path as ElementPath<'a>>::E: ElementHasAttributes<'a>,
    {
        match self.node() {
            Node::Text(ref unescaped) => {
                let bytes_text = BytesText::new(unescaped);
                Event::Text(bytes_text)
            }
            Node::DocType(ref text) => Event::DocType(BytesText::new(text)),
            Node::Start => {
                let element = self.as_element().unwrap();
                let name = element.name().to_owned();
                let attributes = element.attributes();
                let bytes = BytesStart::new(name).with_attributes(attributes);
                Event::Start(bytes)
            }
            Node::End => {
                let element = self.as_element().unwrap();
                Event::End(BytesEnd::new(element.name()))
            }
        }
    }

    /// The element path, not including the potential current element
    fn context_path(&self) -> Self::Path {
        match self.node() {
            Node::Start | Node::End => {
                let path = self.as_path();
                path.slice(0..(path.len() - 1))
            }
            _ => self.as_path(),
        }
    }

    fn map_all<E2, F>(self, map: F) -> MappedItem<'a, Self, E2, F>
    where
        // E1: Element<'a>,
        E2: Element<'a>,
        F: Fn([&'a (); 0], ElementOfItem<'a, Self>) -> E2 + Clone,
        Self: Sized,
    {
        MappedItem {
            inner: self,
            map,
            _phantom: PhantomData::default(),
        }
    }
}

impl<'a> Item<'a> for RawItem<'a> {
    type Path = RawElementPath<'a>;

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
    fn as_path(&self) -> RawElementPath<'a> {
        self.context
    }

    fn node(&self) -> &Node {
        &self.node
    }
}

/// Maps each element in the path, the node type itself is unchanged
pub struct MappedItem<'a, I, E2, F>
where
    I: Item<'a>,
    // E1: Element<'a>,
    E2: Element<'a>,
    F: Fn([&'a (); 0], ElementOfItem<'a, I>) -> E2,
    Self: Sized,
{
    map: F,
    inner: I,
    _phantom: PhantomData<&'a E2>,
}

impl<'a, I, E2, F> Item<'a> for MappedItem<'a, I, E2, F>
where
    I: Item<'a>,
    // <<I as Item<'a>>::Path as ElementPath<'a>>::E: E1,
    // E1: Element<'a>,
    E2: Element<'a>,
    F: Fn([&'a (); 0], ElementOfItem<'a, I>) -> E2 + Clone,
{
    type Path = MappedPath<'a, <I as Item<'a>>::Path, E2, F>;

    fn node(&self) -> &Node {
        self.inner.node()
    }

    fn as_element(&self) -> Option<E2> {
        let this = self.inner.as_element();
        let f = &self.map;
        match this {
            Some(x) => Some(f([], x)),
            None => None,
        }
    }

    fn as_path(&self) -> Self::Path {
        MappedPath {
            map: self.map.clone(),
            inner: self.inner.as_path(),
            _phantom: PhantomData,
        }
    }
}

/// Maps each element in the path, the node type itself is unchanged
pub struct MappedPath<'a, P, E2, F>
where
    P: ElementPath<'a>,
    // E1: Element<'a>,
    E2: Element<'a>,
    F: Fn([&'a (); 0], ElementOfPath<'a, P>) -> E2,
    Self: Sized,
{
    map: F,
    inner: P,
    _phantom: PhantomData<&'a E2>,
}

impl<'a, P, E2, F> Clone for MappedPath<'a, P, E2, F>
where
    P: ElementPath<'a>,
    // E1: Element<'a>,
    E2: Element<'a>,
    F: Fn([&'a (); 0], ElementOfPath<'a, P>) -> E2 + Clone,
    Self: Sized,
{
    fn clone(&self) -> Self {
        Self {
            map: self.map.clone(),
            inner: self.inner.clone(),
            _phantom: self._phantom,
        }
    }
}

impl<'a, P, E2, F> ElementPath<'a> for MappedPath<'a, P, E2, F>
where
    P: ElementPath<'a>,
    // E1: Element<'a>,
    E2: Element<'a>,
    F: Fn([&'a (); 0], ElementOfPath<'a, P>) -> E2 + Clone,
    Self: Sized,
{
    type E = F::Output;

    fn len(&self) -> usize {
        self.inner.len()
    }

    fn get(&self, idx: usize) -> Option<Self::E> {
        let this = self.inner.get(idx);
        let f = &self.map;
        match this {
            Some(x) => Some(f([], x)),
            None => None,
        }
    }

    fn split_last(&self) -> Option<(Self::E, Self)>
    where
        Self: Sized,
    {
        todo!()
    }

    fn slice<I: std::slice::SliceIndex<[NormalisedElement], Output = [NormalisedElement]>>(
        &self,
        // not sure about this type, it looks weird
        _index: I,
    ) -> Self {
        todo!()
    }
}
