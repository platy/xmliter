//! Main things to match: name, class, id, combinations thereof, sequences thereof and options thereof

use html5ever::{tendril::StrTendril, *};

use crate::{HtmlPath, HtmlPathElement};

pub trait Matcher {
    fn is_match<Handle>(
        &self,
        path: HtmlPath<'_, Handle>,
        element: &HtmlPathElement<'_, Handle>,
    ) -> bool;
}

#[derive(Debug, Default)]
pub struct ElementMatcher {
    name: Option<QualName>,
    id: Option<StrTendril>,
    classes: Vec<StrTendril>,
}

impl ElementMatcher {
    pub fn element_match<Handle>(&self, element: &HtmlPathElement<'_, Handle>) -> bool {
        const ID: QualName = QualName {
            prefix: None,
            ns: ns!(),
            local: local_name!("id"),
        };
        self.name
            .as_ref()
            .map_or(true, |match_name| *match_name == element.name)
            && self.id.as_ref().map_or(true, |match_id| {
                element.attr(ID).map_or(false, |id| match_id == id)
            })
            && self
                .classes
                .iter()
                .all(|match_class| element.classes().any(|class| **match_class == *class))
    }

    pub fn class(self, class: StrTendril) -> Self {
        let mut classes = self.classes;
        classes.push(class);
        Self {
            name: self.name,
            id: self.id,
            classes,
        }
    }

    pub fn name(self, local_name: LocalName) -> Self {
        Self {
            name: Some(QualName {
                prefix: None,
                ns: ns!(html),
                local: local_name,
            }),
            id: self.id,
            classes: self.classes,
        }
    }
}

impl Matcher for ElementMatcher {
    fn is_match<Handle>(
        &self,
        _path: HtmlPath<'_, Handle>,
        element: &HtmlPathElement<'_, Handle>,
    ) -> bool {
        self.element_match(element)
    }
}
