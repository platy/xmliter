use std::{fmt, io::BufRead, mem};

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

    pub fn get(&self) -> Option<Item> {
        self.current.as_ref().map(|node| Item {
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
    pub(crate) fn split_last(&self) -> Option<(Element<'a>, ElementPath<'a>)> {
        if let Some((element, path)) = self.path.split_last() {
            Some((
                Element {
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

    pub(crate) fn as_item(&self) -> Option<Item<'a>> {
        if !self.path.is_empty() {
            Some(Item {
                context: *self,
                node: Node::Start,
            })
        } else {
            None
        }
    }

    fn as_element(&self, first: &'a NormalisedElement) -> Element<'a> {
        Element {
            element: first,
            _buf: self.buf,
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
    type Item = Element<'a>;

    type IntoIter = ElementPathIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        ElementPathIter(self)
    }
}

pub struct ElementPathIter<'a>(ElementPath<'a>);

impl<'a> Iterator for ElementPathIter<'a> {
    type Item = Element<'a>;

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
struct NormalisedElement {
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
pub struct Item<'a> {
    context: ElementPath<'a>,
    node: Node,
}

impl<'a> Item<'a> {
    pub fn as_element(&self) -> Option<Element<'a>> {
        match self.node {
            Node::Start => self
                .context
                .path
                .last()
                .map(|en| self.context.as_element(en)),
            Node::End => todo!(),
            _ => None,
        }
    }

    /// The element path, not including the potential current element
    pub(crate) fn into_context_path(self) -> ElementPath<'a> {
        match self.node {
            Node::Start | Node::End => ElementPath {
                path: &self.context.path[0..(self.context.path.len() - 1)],
                buf: self.context.buf,
            },
            _ => self.context,
        }
    }

    /// The element path, not including the potential current element
    pub(crate) fn context_path(&self) -> ElementPath<'_> {
        match self.node {
            Node::Start | Node::End => ElementPath {
                path: &self.context.path[0..(self.context.path.len() - 1)],
                buf: self.context.buf,
            },
            _ => self.context,
        }
    }

    /// The element path, including the element itself if it is one
    pub(crate) fn as_path(&self) -> ElementPath {
        self.context
    }

    pub fn as_event(&self) -> Event<'static> {
        use std::fmt::Write;

        match self.node {
            Node::Text(ref unescaped) => {
                let bytes_text = BytesText::from_escaped_str(unescaped).into_owned();
                Event::Text(bytes_text)
            }
            Node::DocType(ref text) => {
                Event::DocType(BytesText::from_escaped_str(text).into_owned())
            }
            Node::Start => {
                let element = self.context.path.last().unwrap();
                let mut s = element.name.clone();
                let name_len = s.len();
                for NormalisedAttribute { name, value } in &element.attrs {
                    write!(&mut s, r#" {}="{}""#, name, value).unwrap();
                }
                Event::Start(BytesStart::owned(s, name_len))
            }
            Node::End => Event::End(BytesEnd::owned(
                self.context.path.last().unwrap().name.clone().into_bytes(),
            )),
        }
    }
}

impl<'a> std::fmt::Debug for Item<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}/{:?}", self.context, self.node)
    }
}

// should index elements and unescaped text in the path
#[derive(Clone)]
enum Node {
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
pub struct Element<'a> {
    element: &'a NormalisedElement,
    _buf: &'a ElementPathBuf,
}

impl<'a> Element<'a> {
    pub(crate) fn name(&self) -> &str {
        &self.element.name
    }

    pub(crate) fn attributes(&self) -> std::slice::Iter<'_, NormalisedAttribute> {
        self.element.attrs.iter()
    }

    pub fn attr(&self, search: &str) -> Option<&str> {
        for NormalisedAttribute { name, value } in self.attributes() {
            if name == search {
                return Some(value);
            }
        }
        None
    }

    pub fn classes(&self) -> Classes {
        match self.attr("class") {
            Some(s) => Classes { s },
            None => Classes { s: "" },
        }
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
