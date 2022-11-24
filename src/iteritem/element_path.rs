//! Core parts of representing an element and it's ancestral path
use std::{fmt, io::BufRead, slice::SliceIndex};

use quick_xml::{events::BytesStart, name::QName, Reader};

/// An owned path of elements
#[derive(Debug, Clone)]
pub struct ElementPathBuf {
    pub(crate) path: Vec<NormalisedElement>,
}

impl ElementPathBuf {
    pub(crate) fn new() -> Self {
        Self { path: vec![] }
    }

    pub(crate) fn text(&self, text: String) -> Node {
        Node::Text(text)
    }

    pub(crate) fn doctype(&self, text: String) -> Node {
        Node::DocType(text)
    }

    pub(crate) fn start<B: BufRead>(&mut self, start: BytesStart, reader: &Reader<B>) -> Node {
        let decoder = reader.decoder();
        let element = NormalisedElement {
            name: decoder.decode(start.name().as_ref()).unwrap().to_string(),
            attrs: start
                .attributes()
                .map(|a| {
                    let a = a.unwrap();
                    NormalisedAttribute {
                        name: decoder.decode(a.key.as_ref()).unwrap().to_string(),
                        value: decoder.decode(&a.value).unwrap().to_string(),
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

    pub(crate) fn end(&self) -> Node {
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
    pub(crate) path: &'a [NormalisedElement],
    pub(crate) buf: &'a ElementPathBuf,
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

    pub(crate) fn as_element(&self, first: &'a NormalisedElement) -> RawElement<'a> {
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
    pub(crate) name: String,
    pub(crate) attrs: Vec<NormalisedAttribute>,
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

impl<'a> From<&'a NormalisedAttribute> for quick_xml::events::attributes::Attribute<'a> {
    fn from(NormalisedAttribute { name, value }: &'a NormalisedAttribute) -> Self {
        let key = QName(name.as_bytes());
        let value = value.as_bytes().into();
        quick_xml::events::attributes::Attribute { key, value }
    }
}

/// An item in the traversal, with access to the current node and the context of elements
pub struct RawItem<'a> {
    pub(crate) context: ElementPath<'a>,
    pub(crate) node: Node,
}

impl<'a> std::fmt::Debug for RawItem<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}/{:?}", self.context, self.node)
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
    pub(crate) element: &'a NormalisedElement,
    pub(crate) _buf: &'a ElementPathBuf,
}

impl<'a> RawElement<'a> {
    pub(crate) fn attributes(&self) -> std::slice::Iter<'_, NormalisedAttribute> {
        self.element.attrs.iter()
    }
}
