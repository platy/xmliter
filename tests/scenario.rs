use std::io::{BufReader, Cursor};

use xmliter::{HtmlIter, HtmlIterator, HtmlWriter, css_select, selector::ContextualSelector};

#[test]
fn include_chain() {
    let read = BufReader::new(Cursor::new("<!DOCTYPE html><html><body><main>content</main></body></html>"));
    let out = HtmlIter::from_reader(read).include(css_select!("main")).to_string();
    assert_eq!(out, "<main>content</main>");
}

#[test]
fn include_for() {
    let read = BufReader::new(Cursor::new("<!DOCTYPE html><html><body><main>content</main></body></html>"));
    let mut iter = HtmlIter::from_reader(read);
    let mut out = Vec::new();
    let mut writer = HtmlWriter::from_writer(&mut out);
    while let Some(item) = iter.next() {
        if let Some(under_main) = item.include(&css_select!("main")) { // anything without main in the path is ignored, and any context with main is stripped before the main
            writer.write_item(under_main);
        }
    }
    assert_eq!(String::from_utf8(out).unwrap(), "<main>content</main>");
}

#[test]
fn exclude_chain() {
    let read = BufReader::new(Cursor::new(r#"<!DOCTYPE html><html><body><main><p>content</p><p class="bloat">bloat</p></main></body></html>"#));
    let out = HtmlIter::from_reader(read).exclude(css_select!(("main") (."bloat"))).to_string();
    assert_eq!(out, "<!DOCTYPE html><html><body><main><p>content</p></main></body></html>");
}

#[test]
fn exclude_for() {
    let read = BufReader::new(Cursor::new(r#"<!DOCTYPE html><html><body><main><p>content</p><p class="bloat">bloat</p></main></body></html>"#));
    let mut iter = HtmlIter::from_reader(read);
    let mut out = Vec::new();
    let mut writer = HtmlWriter::from_writer(&mut out);
    while let Some(item) = iter.next() {
        if !css_select!(("main") (."bloat")).match_any(item.as_path()) { // if nothing in the item's path matches
            writer.write_item(item);
        }
    }
    assert_eq!(String::from_utf8(out).unwrap(), "<!DOCTYPE html><html><body><main><p>content</p></main></body></html>");
}
