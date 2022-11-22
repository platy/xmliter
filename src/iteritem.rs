use std::{fmt, io::BufRead, mem, slice::SliceIndex};

use quick_xml::{
    events::{BytesEnd, BytesStart, BytesText, Event},
    Reader,
};

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

/// An owned path of elements
#[derive(Debug, Clone)]
pub struct ElementPathBuf {
    path: Vec<NormalisedElement>,
}

impl ElementPathBuf {
    pub(crate) fn new() -> Self {
        Self { path: vec![] }
    }

    fn text(&self, text: String) -> Node {
        Node::Text(text)
    }

    fn doctype(&self, text: String) -> Node {
        Node::DocType(text)
    }

    fn start<B: BufRead>(&mut self, start: BytesStart, reader: &Reader<B>) -> Node {
        let element = NormalisedElement {
            name: reader.decode(start.name()).unwrap().to_string(),
            attrs: start
                .attributes()
                .map(|a| {
                    let a = a.unwrap();
                    NormalisedAttribute {
                        name: reader.decode(a.key).unwrap().to_string(),
                        value: reader.decode(&a.value).unwrap().to_string(),
                    }
                })
                .collect(),
        };
        self.path.push(element);
        Node::Start
    }

    #[cfg(test)]
    pub(crate) fn append_element(&mut self, name: &str, attr: Vec<(&str, &str)>) -> &mut Self {
        let element = NormalisedElement {
            name: name.to_string(),
            attrs: attr
                .into_iter()
                .map(|(name, value)| NormalisedAttribute {
                    name: name.to_string(),
                    value: value.to_string(),
                })
                .collect(),
        };
        self.path.push(element);
        self
    }

    fn end(&self) -> Node {
        Node::End
    }

    pub(crate) fn as_path(&self) -> ElementPath {
        ElementPath {
            path: &self.path,
            buf: self,
        }
    }
}

/// A path of elements
#[derive(Clone, Copy)]
pub struct ElementPath<'a> {
    path: &'a [NormalisedElement],
    buf: &'a ElementPathBuf,
}

impl<'a> ElementPath<'a> {
    pub fn len(&self) -> usize {
        self.path.len()
    }

    pub(crate) fn split_last(&self) -> Option<(RawElement<'a>, ElementPath<'a>)> {
        if let Some((element, path)) = self.path.split_last() {
            Some((
                RawElement {
                    element,
                    _buf: self.buf,
                },
                ElementPath {
                    path,
                    buf: self.buf,
                },
            ))
        } else {
            None
        }
    }

    pub(crate) fn as_item(&self) -> Option<RawItem<'a>> {
        if !self.path.is_empty() {
            Some(RawItem {
                context: *self,
                node: Node::Start,
            })
        } else {
            None
        }
    }

    fn as_element(&self, first: &'a NormalisedElement) -> RawElement<'a> {
        RawElement {
            element: first,
            _buf: self.buf,
        }
    }

    pub(crate) fn slice<I: SliceIndex<[NormalisedElement], Output = [NormalisedElement]>>(
        &self,
        index: I,
    ) -> Self {
        Self {
            path: &self.path[index],
            buf: self.buf,
        }
    }
}

impl<'a> fmt::Debug for ElementPath<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for element in self.path {
            write!(f, "/{:?}", element)?;
        }
        Ok(())
    }
}

impl<'a> IntoIterator for ElementPath<'a> {
    type Item = RawElement<'a>;

    type IntoIter = ElementPathIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        ElementPathIter(self)
    }
}

pub struct ElementPathIter<'a>(ElementPath<'a>);

impl<'a> Iterator for ElementPathIter<'a> {
    type Item = RawElement<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some((first, rest)) = self.0.path.split_first() {
            self.0.path = rest;
            Some(self.0.as_element(first))
        } else {
            None
        }
    }
}

impl<'a> DoubleEndedIterator for ElementPathIter<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if let Some((last, rest)) = self.0.path.split_last() {
            self.0.path = rest;
            Some(self.0.as_element(last))
        } else {
            None
        }
    }
}

/// Currently Heap allocated, but to be fixed size with no references, instead should only contain slice index ranges into vecs stored on element paths
#[derive(Clone)]
pub(crate) struct NormalisedElement {
    name: String,
    attrs: Vec<NormalisedAttribute>,
}

impl fmt::Debug for NormalisedElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)?;
        for a in &self.attrs {
            write!(f, " {}=\"{}\"", a.name, a.value)?;
        }
        Ok(())
    }
}

/// Currently Heap allocated, but to be fixed size with no references, instead should only contain slice index ranges into vecs stored on element paths
#[derive(Clone, Debug)]
pub(crate) struct NormalisedAttribute {
    pub(crate) name: String,
    pub(crate) value: String,
}

/// An item in the traversal, with access to the current node and the context of elements
pub struct RawItem<'a> {
    pub(crate) context: ElementPath<'a>,
    pub(crate) node: Node,
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

impl<'a> std::fmt::Debug for RawItem<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}/{:?}", self.context, self.node)
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

// should index elements and unescaped text in the path. Wanted it to be private, maybe it still can be
#[derive(Clone)]
pub enum Node {
    DocType(String),
    Start,
    End,
    Text(String),
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DocType(arg) => write!(f, "DOCTYPE {}", arg),
            Self::Start => write!(f, "Start"),
            Self::End => write!(f, "End"),
            Self::Text(arg) => fmt::Debug::fmt(&arg, f),
        }
    }
}

/// An element in the context
pub struct RawElement<'a> {
    element: &'a NormalisedElement,
    _buf: &'a ElementPathBuf,
}

impl<'a> RawElement<'a> {
    pub(crate) fn attributes(&self) -> std::slice::Iter<'_, NormalisedAttribute> {
        self.element.attrs.iter()
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
