use std::ops::RangeFrom;

use crate::{
    iteritem::{ElementPath, Node, RawElement, RawElementPath, RawItem},
    selector::ContextualSelector,
    Item,
};

/// Extensions for operations on an item which use selectors, currently just to avoid circular dependencies
pub trait ItemExt {
    /// Filters this item by the selector. If an element in the context is found to match the selector, returns `Some` with the context starting at that element, if it matches the current element, returns that with no context and otherwise returns `None`
    fn include(self, selector: &dyn ContextualSelector) -> Option<IncludeItem<Self>>
    where
        Self: Sized;
}

impl<'a, T> ItemExt for T
where
    T: Item<'a>,
{
    fn include(self, selector: &dyn ContextualSelector) -> Option<IncludeItem<Self>>
    where
        Self: Sized,
    {
        let path = self.as_path();
        for start in 0..path.len() {
            let item = RawItem {
                context: path.slice(..=start),
                node: Node::Start,
            };
            if selector.context_match(&item) {
                let item = IncludeItem {
                    range: start..,
                    inner: self,
                };
                return Some(item);
            }
        }
        None
    }
}

pub struct IncludeItem<I> {
    range: RangeFrom<usize>,
    inner: I,
}

impl<'a, I: Item<'a>> Item<'a> for IncludeItem<I> {
    fn as_element(&self) -> Option<RawElement<'a>> {
        todo!()
    }

    fn as_path(&self) -> RawElementPath<'a> {
        self.inner.as_path().slice(self.range.clone()) // feels like this could be stored instead of the range
    }

    /// maybe this shouldn't be exposed at all we currenlty only need it to know whether this is an element, something needed for as_element too
    fn node(&self) -> &Node {
        self.inner.node()
    }
}
