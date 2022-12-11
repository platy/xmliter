use std::io::{BufReader, Cursor};

use lending_iterator::HKT;
use xmliter::*;

#[test]
fn include_chain() {
    let read = BufReader::new(Cursor::new(
        "<!DOCTYPE html><html><body><main>content</main></body></html>",
    ));
    let out = HtmlIter::from_reader(read)
        .include(css_select!("main"))
        .to_string();
    assert_eq!(out, "<main>content</main>");
}

#[test]
fn include_for() {
    let read = BufReader::new(Cursor::new(
        "<!DOCTYPE html><html><body><main>content</main></body></html>",
    ));
    let mut iter = HtmlIter::from_reader(read);
    let mut out = Vec::new();
    let mut writer = HtmlWriter::from_writer(&mut out);
    while let Some(item) = iter.next() {
        if let Some(under_main) = item.include(&css_select!("main")) {
            // anything without main in the path is ignored, and any context with main is stripped before the main
            writer.write_item(&under_main);
        }
    }
    assert_eq!(String::from_utf8(out).unwrap(), "<main>content</main>");
}

#[test]
fn exclude_chain() {
    let read = BufReader::new(Cursor::new(
        r#"<!DOCTYPE html><html><body><main><p>content</p><p class="bloat">bloat</p></main></body></html>"#,
    ));
    let out = HtmlIter::from_reader(read)
        .exclude(css_select!(("main") (."bloat")))
        .to_string();
    assert_eq!(
        out,
        "<!DOCTYPE html><html><body><main><p>content</p></main></body></html>"
    );
}

#[test]
fn exclude_for() {
    let read = BufReader::new(Cursor::new(
        r#"<!DOCTYPE html><html><body><main><p>content</p><p class="bloat">bloat</p></main></body></html>"#,
    ));
    let mut iter = HtmlIter::from_reader(read);
    let mut out = Vec::new();
    let mut writer = HtmlWriter::from_writer(&mut out);
    while let Some(item) = iter.next() {
        if !css_select!(("main") (."bloat")).match_any(item.as_path()) {
            // if nothing in the item's path matches
            writer.write_item(&item);
        }
    }
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "<!DOCTYPE html><html><body><main><p>content</p></main></body></html>"
    );
}

#[test]
fn mutate_chain() {
    let read = BufReader::new(Cursor::new(
        r#"<!DOCTYPE html><html><body><main id="id"><p id="first">content</p><p class="bloat" id="second">bloat</p></main></body></html>"#,
    ));
    let out = HtmlIter::from_reader(read)
        .map_all::<HKT!(FilterAttributes<RawElement<'_>, _>), _>(
            |_, element| // on the iterator, `map_all` means that this mapping applies to every element in the document
                element.filter_attributes(|name, _value|name != "id"),
        )
        .to_string(); // the attributes are a regular `std::iter::Iterator`
    assert_eq!(
        out,
        r#"<!DOCTYPE html><html><body><main><p>content</p><p class="bloat">bloat</p></main></body></html>"#
    );
}

#[test]
fn mutate_for() {
    let read = BufReader::new(Cursor::new(
        r#"<!DOCTYPE html><html><body><main id="id"><p id="first">content</p><p class="bloat" id="second">bloat</p></main></body></html>"#,
    ));
    let mut iter = HtmlIter::from_reader(read);
    let mut out = Vec::new();
    let mut writer = HtmlWriter::from_writer(&mut out);
    while let Some(item) = iter.next() {
        let item = item.map_all(
            |_, element| // on the item, `map_all` means this mapping applies to every element in the path
            element.filter_attributes(|name, _value| name != "id"),
        );
        writer.write_item(&item);
    }
    assert_eq!(
        String::from_utf8(out).unwrap(),
        r#"<!DOCTYPE html><html><body><main><p>content</p><p class="bloat">bloat</p></main></body></html>"#
    );
}
