use crate::iteritem::{Element, ElementPath, Item};

/// Selects elements using a syntax similar to css 1 selectors, supporting css 1 selectors except pseudo-elements and pseudo classes
///
/// ```
/// use xmliter::css_select;
///
/// css_select!("p");
/// css_select!("p"."quote");
/// css_select!(."quote");
/// css_select!(#"main");
/// css_select!((#"main") ("p"."quote"));
/// ```
#[macro_export]
macro_rules! css_select {
    (@inner [($($head:tt)+)] -> [$selector:expr]) => {
        ($crate::selector::ContextualSelectCons($selector , css_select!($($head)+)))
    };
    (@inner [($($head:tt)+) $($tail:tt)*] -> [$selector:expr]) => {
        css_select!(@inner [$($tail)*] -> [$crate::selector::ContextSelectCons($selector , css_select!($($head)+))])
    };
    ($(($($selectors:tt)+))+) => {
        css_select!(@inner [$(($($selectors)+))+] -> [$crate::selector::MatchAll])
    };
    ($name:literal.$class:literal) => {
        $crate::selector::Selector::and(
            $crate::selector::NameSelector($name),
            $crate::selector::ClassSelector($class),
        )
    };
    ($name:literal#$id:literal) => {
        $crate::selector::Selector::and(
            $crate::selector::NameSelector($name),
            $crate::selector::IdSelector($id),
        )
    };
    ($name:literal) => {
        $crate::selector::NameSelector($name)
    };
    (.$class:literal) => {
        $crate::selector::ClassSelector($class)
    };
    (#$id:literal) => {
        $crate::selector::IdSelector($id)
    };
}

pub trait Selector {
    fn is_match(&self, element: &Element<'_>) -> bool;

    fn and<O: Selector>(self, other: O) -> AndSelector<Self, O>
    where
        Self: Sized,
    {
        AndSelector(self, other)
    }
}

pub trait ContextualSelector {
    fn context_match(&self, item: &Item<'_>) -> bool;

    fn match_any(&self, mut context: ElementPath<'_>) -> bool {
        while let Some(item) = context.as_item() {
            if self.context_match(&item) {
                return true;
            }
            context = item.into_context_path();
        }
        false
    }

    fn or<O: ContextualSelector>(self, other: O) -> GroupSelector<Self, O>
    where
        Self: Sized,
    {
        GroupSelector(self, other)
    }
}

pub trait OnlyContextualSelector {
    fn match_any(&self, context: ElementPath) -> bool;
}

impl<S> ContextualSelector for S
where
    S: Selector,
{
    fn context_match(&self, item: &Item<'_>) -> bool {
        item.as_element()
            .map_or(false, |element| self.is_match(&element))
    }
}

pub struct NameSelector(pub &'static str);

impl Selector for NameSelector {
    fn is_match(&self, element: &Element<'_>) -> bool {
        *self.0 == *element.name()
    }
}

pub struct ClassSelector(pub &'static str);

impl Selector for ClassSelector {
    fn is_match(&self, element: &Element<'_>) -> bool {
        element.classes().any(|class| class == self.0)
    }
}

pub struct IdSelector(pub &'static str);

impl Selector for IdSelector {
    fn is_match(&self, element: &Element<'_>) -> bool {
        element
            .attributes()
            .any(|attr| attr.name == "id" && attr.value == self.0)
    }
}

/// A contextual selector, the last selector must match the element exactly and the preceding must match elements in the context in that order
impl<S: Selector> ContextualSelector for [S] {
    fn context_match(&self, item: &Item<'_>) -> bool {
        let mut to_match = self.iter().rev();
        if let Some(end_matcher) = to_match.next() {
            if !item
                .as_element()
                .map_or(false, |element| end_matcher.is_match(&element))
            {
                return false;
            }
        } else {
            return true;
        }
        let mut path = item.context_path().into_iter().rev();
        'outer: for matcher in to_match {
            for element in &mut path {
                if matcher.is_match(&element) {
                    continue 'outer;
                }
            }
            return false;
        }
        true
    }
}

/// Always matches
pub struct MatchAll;

impl Selector for MatchAll {
    fn is_match(&self, _element: &Element<'_>) -> bool {
        true
    }
}

impl OnlyContextualSelector for MatchAll {
    fn match_any(&self, _context: ElementPath<'_>) -> bool {
        true
    }
}

/// Matches something in the context, then continues by using the second matcher for the remaining context
pub struct ContextSelectCons<C, A>(pub C, pub A);

impl<C: OnlyContextualSelector, A: Selector> OnlyContextualSelector for ContextSelectCons<C, A> {
    fn match_any(&self, mut context: ElementPath<'_>) -> bool {
        while let Some((last, rest)) = context.split_last() {
            let element = last;
            if self.1.is_match(&element) {
                return self.0.match_any(rest);
            }
            context = rest;
        }
        false
    }
}

/// Matches the element, then continues by using the second matcher for the remaining context
pub struct ContextualSelectCons<C: OnlyContextualSelector, A: Selector>(pub C, pub A);

impl<C: OnlyContextualSelector, A: Selector> ContextualSelector for ContextualSelectCons<C, A> {
    fn context_match<'a>(&self, item: &Item<'a>) -> bool {
        item.as_element()
            .map_or(false, |element| self.1.is_match(&element))
            && self.0.match_any(item.as_path())
    }
}

/// Groups together 2 selectors, selects elements that either would select
pub struct GroupSelector<A: ContextualSelector, B: ContextualSelector>(A, B);

impl<A: ContextualSelector, B: ContextualSelector> ContextualSelector for GroupSelector<A, B> {
    fn context_match(&self, item: &Item<'_>) -> bool {
        self.0.context_match(item) || self.1.context_match(item)
    }
}

/// COmbines 2 selectors, selecting something taht both would select
pub struct AndSelector<A: Selector, B: Selector>(A, B);

impl<A: Selector, B: Selector> Selector for AndSelector<A, B> {
    fn is_match(&self, element: &Element<'_>) -> bool {
        self.0.is_match(element) && self.1.is_match(element)
    }
}

#[test]
fn test_matchers() {
    let mut path_body = crate::iteritem::ElementPathBuf::new();
    path_body
        .append_element("html", vec![])
        .append_element("body", vec![]);
    let mut path_main = path_body.clone();
    path_main.append_element("div", vec![("id", "main")]);
    let mut main_p = path_main.clone();
    main_p.append_element("p", vec![]);
    let mut main_quote = path_main.clone();
    main_quote.append_element("p", vec![("class", "fixed quote")]);
    let mut body_quote = path_body.clone();
    body_quote.append_element("p", vec![("class", "fixed quote")]);

    assert!(css_select!("p").context_match(&main_p.as_path().as_item().unwrap()));
    assert!(css_select!("p").context_match(&main_quote.as_path().as_item().unwrap()));
    assert!(!css_select!("p").context_match(&path_main.as_path().as_item().unwrap()));

    assert!(!css_select!("p"."quote").context_match(&main_p.as_path().as_item().unwrap()));
    assert!(css_select!("p"."quote").context_match(&main_quote.as_path().as_item().unwrap()));
    assert!(!css_select!("p"."quote").context_match(&path_main.as_path().as_item().unwrap()));

    assert!(!css_select!(."quote").context_match(&main_p.as_path().as_item().unwrap()));
    assert!(css_select!(."quote").context_match(&main_quote.as_path().as_item().unwrap()));
    assert!(!css_select!(."quote").context_match(&path_main.as_path().as_item().unwrap()));

    assert!(!css_select!(#"main").context_match(&main_p.as_path().as_item().unwrap()));
    assert!(!css_select!(#"main").context_match(&main_quote.as_path().as_item().unwrap()));
    assert!(css_select!(#"main").context_match(&path_main.as_path().as_item().unwrap()));

    assert!(
        !css_select!((#"main") ("p"."quote")).context_match(&main_p.as_path().as_item().unwrap())
    );
    assert!(css_select!((#"main") ("p"."quote"))
        .context_match(&main_quote.as_path().as_item().unwrap()));
    assert!(!css_select!((#"main") ("p"."quote"))
        .context_match(&path_main.as_path().as_item().unwrap()));
    assert!(!css_select!((#"main") ("p"."quote"))
        .context_match(&body_quote.as_path().as_item().unwrap()));
}
