use std::marker::PhantomData;

use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};

mod element;
mod element_path;

pub use self::element::Element;
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
    fn node(&self) -> &Node;

    fn as_element(&self) -> Option<RawElement<'a>>;

    // /// The element path, not including the potential current element
    // pub(crate) fn into_context_path(self) -> ElementPath<'a>;

    // /// The element path, not including the potential current element
    // pub(crate) fn context_path(&self) -> ElementPath<'_>;

    /// The element path, including the element itself if it is one
    fn as_path(&self) -> ElementPath<'a>;

    /// As a quick-xml event, for serialisation, allocates for start tags but not for others
    fn as_event<'b>(&'b self) -> Event<'_>
    where
        'a: 'b,
    {
        match self.node() {
            Node::Text(ref unescaped) => {
                let bytes_text = BytesText::new(unescaped);
                Event::Text(bytes_text)
            }
            Node::DocType(ref text) => Event::DocType(BytesText::new(text)),
            Node::Start => {
                let element = self.as_path().path.last().unwrap();
                Event::Start(BytesStart::new(&element.name).with_attributes(&element.attrs))
            }
            Node::End => {
                let element = self.as_path().path.last().unwrap();
                Event::End(BytesEnd::new(&element.name))
            }
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

    fn map_all<E1, E2, F>(self, map: F) -> MappedItem<'a, Self, E1, E2, F>
    where
        E1: Element,
        E2: Element,
        F: Fn(&E1) -> E2,
        Self: Sized,
    {
        MappedItem {
            inner: self,
            _map: map,
            _phantom: PhantomData::default(),
        }
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

/// Maps each element in the path, the node type itself is unchanged
pub struct MappedItem<'a, I, E1, E2, F>
where
    I: Item<'a>,
    E1: Element,
    E2: Element,
    F: Fn(&'a E1) -> E2,
    Self: Sized,
{
    _map: F,
    inner: I,
    _phantom: PhantomData<&'a (E1, E2)>,
}

impl<'a, I, E1, E2, F> Item<'a> for MappedItem<'a, I, E1, E2, F>
where
    I: Item<'a>,
    E1: Element,
    E2: Element,
    F: Fn(&E1) -> E2,
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
