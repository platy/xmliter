use std::mem;

use crate::iteritem::element_path::{NormalisedAttribute, RawElement};

pub trait Element<'a> {
    fn name(&self) -> &'a str;

    fn attr(&self, search: &str) -> Option<&str>;

    fn classes(&self) -> Classes {
        match self.attr("class") {
            Some(s) => Classes { s },
            None => Classes { s: "" },
        }
    }

    fn filter_attributes<F>(self, predicate: F) -> FilterAttributes<Self, F>
    where
        Self: Sized,
        F: Fn(&str, &str) -> bool,
    {
        FilterAttributes {
            inner: self,
            predicate,
        }
    }
}
pub trait ElementHasAttributes<'a> {
    type Attribute: Into<quick_xml::events::attributes::Attribute<'a>>;
    type Attributes: Iterator<Item = Self::Attribute>;
    fn attributes(&self) -> Self::Attributes;
}

impl<'a> Element<'a> for RawElement<'a> {
    fn name(&self) -> &'a str {
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
    pub(crate) s: &'a str,
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

pub struct FilterAttributes<I, P: Fn(&str, &str) -> bool> {
    pub(crate) inner: I,
    pub(crate) predicate: P,
}

impl<'a, I, P> Element<'a> for FilterAttributes<I, P>
where
    I: Element<'a>,
    P: Fn(&str, &str) -> bool,
{
    fn name(&self) -> &'a str {
        self.inner.name()
    }

    fn attr(&self, search: &str) -> Option<&str> {
        self.inner
            .attr(search)
            .and_then(|value| (self.predicate)(search, value).then_some(value))
    }
}
