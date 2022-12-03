use std::ops::RangeFrom;

use crate::{
    iteritem::{ElementPath, Node},
    selector::ContextualSelector,
    Item,
};

/// Extensions for operations on an item which use selectors, currently just to avoid circular dependencies
pub trait ItemExt {
    /// Filters this item by the selector. If an element in the context is found to match the selector, returns `Some` with the context starting at that element, if it matches the current element, returns that with no context and otherwise returns `None`
    fn include(self, selector: &impl ContextualSelector) -> Option<IncludeItem<Self>>
    where
        Self: Sized;
}

impl<'a, T> ItemExt for T
where
    T: Item<'a>,
{
    fn include(self, selector: &impl ContextualSelector) -> Option<IncludeItem<Self>>
    where
        Self: Sized,
    {
        let path = self.as_path();
        for start in 0..path.len() {
            if selector.context_match(path.slice(..=start)) {
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

/// Wraps item in a filter on it's path which elides the first part of it's path
pub struct IncludeItem<I> {
    range: RangeFrom<usize>,
    inner: I,
}

impl<'a, I: Item<'a>> Item<'a> for IncludeItem<I> {
    type Path = <I as Item<'a>>::Path;

    fn as_element(&self) -> Option<<Self::Path as ElementPath<'a>>::E> {
        self.inner.as_element()
    }

    fn as_path(&self) -> Self::Path {
        self.inner.as_path().slice(self.range.clone())
    }

    /// maybe this shouldn't be exposed at all we currenlty only need it to know whether this is an element, something needed for as_element too
    fn node(&self) -> &Node {
        self.inner.node()
    }
}
