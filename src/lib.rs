#![allow(clippy::manual_map)] // have ben having some trouble with `Option::map` with these HKT's

use std::{
    io::{self, BufRead, Cursor},
    marker::PhantomData,
};

mod itemext;
mod iteritem;
pub mod selector;

pub use itemext::{IncludeItem, ItemExt};
pub use iteritem::{Element, FilterAttributes, Item, RawElement, RawItem};
use lending_iterator::prelude::{Apply, HKT};
pub use selector::ContextualSelector;

use iteritem::{ElementHasAttributes, ElementPath, MappedItem, Traverser};

type ElementOfPath<'a, Path> = <Path as ElementPath<'a>>::E;
type ElementOfItem<'a, I> = ElementOfPath<'a, <I as Item<'a>>::Path>;
type ElementOfIterator<'a, It> = ElementOfItem<'a, <It as HtmlIterator>::Item<'a>>;

pub trait HtmlIterator {
    type Item<'a>: Item<'a>
    where
        Self: 'a;

    fn next(&mut self) -> Option<Self::Item<'_>> {
        self.advance();
        self.get()
    }

    fn advance(&mut self);

    fn get(&self) -> Option<Self::Item<'_>>;

    fn map_all<E2, F>(self, map: F) -> MappedItems<Self, E2, F>
    where
        E2: HKT,
        for<'any> Apply!(E2<'any>): Element<'any>,
        for<'a> F: Fn([&'a (); 0], ElementOfIterator<'a, Self>) -> Apply!(E2<'a>) + Clone,
        Self: Sized,
    {
        MappedItems {
            inner: self,
            map,
            _phantom: PhantomData::default(),
        }
    }

    fn exclude<S: ContextualSelector>(self, selector: S) -> Exclude<Self, S>
    where
        Self: Sized,
    {
        Exclude {
            inner: self,
            selector,
        }
    }

    fn include<S: ContextualSelector>(self, selector: S) -> Include<Self, S>
    where
        Self: Sized,
    {
        Include {
            inner: self,
            selector,
        }
    }

    fn write_into(mut self, f: impl io::Write)
    where
        Self: Sized,
        for<'a> <<Self::Item<'a> as Item<'a>>::Path as ElementPath<'a>>::E:
            ElementHasAttributes<'a>,
    {
        let mut writer = HtmlWriter::from_writer(f);
        while let Some(item) = self.next() {
            writer.write_item(&item);
        }
    }

    fn to_string(self) -> String
    where
        Self: Sized,
        for<'a> <<Self::Item<'a> as Item<'a>>::Path as ElementPath<'a>>::E:
            ElementHasAttributes<'a>,
    {
        let mut buf = vec![];
        self.write_into(Cursor::new(&mut buf));
        String::from_utf8(buf).unwrap()
    }
}

pub struct MappedItems<It, E2, F>
where
    It: HtmlIterator,
    E2: HKT,
    for<'any> Apply!(E2<'any>): Element<'any>,
    for<'a> F: Fn([&'a (); 0], ElementOfIterator<'a, It>) -> Apply!(E2<'a>) + Clone,
{
    inner: It,
    map: F,
    _phantom: PhantomData<E2>,
}

impl<It, E2, F> HtmlIterator for MappedItems<It, E2, F>
where
    It: HtmlIterator,
    E2: HKT,
    for<'any> Apply!(E2<'any>): Element<'any>,
    for<'a> F: Fn([&'a (); 0], ElementOfIterator<'a, It>) -> Apply!(E2<'a>) + Clone,
{
    type Item<'a> = MappedItem<'a, It::Item<'a>, Apply!(E2<'a>), F>
    where
        Self: 'a;

    fn advance(&mut self) {
        self.inner.advance()
    }

    fn get(&self) -> Option<Self::Item<'_>> {
        // Interestingly, this doesn't work using `Option::Map`
        match self.inner.get() {
            Some(i) => Some(Item::map_all(i, self.map.clone())),
            None => None,
        }
    }
}

pub struct HtmlWriter<W: io::Write> {
    inner: quick_xml::Writer<W>,
}

impl<W: io::Write> HtmlWriter<W> {
    pub fn from_writer(writer: W) -> Self {
        Self {
            inner: quick_xml::Writer::new(writer),
        }
    }

    pub fn write_item<'e, I>(&mut self, item: &I)
    where
        I: Item<'e>,
        ElementOfItem<'e, I>: ElementHasAttributes<'e>,
    {
        self.inner.write_event(&item.as_event()).unwrap();
    }
}

pub struct HtmlIter<B: BufRead> {
    reader: quick_xml::Reader<B>,
    buf: Traverser,
}

impl<B: BufRead> HtmlIter<B> {
    pub fn from_reader(reader: B) -> Self {
        Self {
            reader: quick_xml::Reader::from_reader(reader),
            buf: Traverser::new(),
        }
    }
}

impl<B: io::BufRead> HtmlIterator for HtmlIter<B> {
    type Item<'a> = RawItem<'a>
    where
        Self: 'a;

    fn advance(&mut self) {
        self.buf.read_from(&mut self.reader)
    }
    fn get(&self) -> Option<Self::Item<'_>> {
        self.buf.get()
    }
}

pub struct Exclude<I, S> {
    inner: I,
    selector: S,
}

impl<I: HtmlIterator, S: ContextualSelector> HtmlIterator for Exclude<I, S> {
    type Item<'a> = I::Item<'a>
    where
        Self: 'a;

    fn advance(&mut self) {
        while let Some(item) = self.inner.next() {
            if !self.selector.match_any(item.as_path()) {
                // if nothing in the item's path matches
                return;
            } else {
                drop(item)
            }
        }
    }

    fn get(&self) -> Option<Self::Item<'_>> {
        self.inner.get()
    }
}

pub struct Include<I, S> {
    inner: I,
    selector: S,
}

impl<I: HtmlIterator, S: ContextualSelector> HtmlIterator for Include<I, S> {
    type Item<'a> = IncludeItem<I::Item<'a>>
    where
        Self: 'a,;

    fn advance(&mut self) {
        while let Some(item) = self.inner.next() {
            if let Some(_item) = item.include(&self.selector) {
                return;
            }
        }
    }

    fn get(&self) -> Option<Self::Item<'_>> {
        self.inner.get().map(|i| i.include(&self.selector).unwrap())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn doc_identity() {
        let test = r#"<!DOCTYPE html><html><head></head><body><p class="hello"><b>hello</b></p><p>world!</p></body></html>"#;
        let out = HtmlIter::from_reader(test.as_bytes());
        assert_eq!(&out.to_string(), test);
    }

    #[test]
    fn fragment_identity() {
        let test = "<p><b>hello</b></p><p>world!</p>";
        let out = HtmlIter::from_reader(test.as_bytes());
        assert_eq!(&out.to_string(), test);
    }

    #[test]
    fn remove_elements() {
        let test = r#"<!DOCTYPE html><html><head></head><body><p class="hello"><b>hello</b></p><p>world!</p></body></html>"#;
        let out = HtmlIter::from_reader(test.as_bytes()).exclude(css_select!(."hello"));
        assert_eq!(
            &out.to_string(),
            r#"<!DOCTYPE html><html><head></head><body><p>world!</p></body></html>"#
        );
    }

    #[test]
    fn select_element() {
        let test = r#"<!DOCTYPE html><html><head></head><body><div id="main"><p><b>hello</b></p><p>world!</p></div><p>side</p></body></html>"#;
        let out = HtmlIter::from_reader(test.as_bytes()).include(css_select!((#"main") ("p")));
        assert_eq!(&out.to_string(), "<p><b>hello</b></p><p>world!</p>");
    }
}
