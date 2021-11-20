use std::{io::{self, BufRead, Cursor}};

// pub mod selector;

// use selector::{ContextualSelector, Selector};

pub trait HtmlIterator {
    fn next(&mut self) -> Option<quick_xml::events::Event>;

    // fn exclude(&self, selector: impl ContextualSelector) -> HtmlIter {
    //     todo!()
    // }

    // fn include(&self, selector: impl ContextualSelector) -> HtmlIter {
    //     todo!()
    // }

    fn write_into(mut self, f: impl io::Write) where Self: Sized {
        let mut writer = quick_xml::Writer::new(f);
        while let Some(event) = self.next() {
            assert!(writer.write_event(&event).is_ok());
        }
    }

    fn to_string(self) -> String where Self: Sized {
        let mut buf = vec![];
        self.write_into(Cursor::new(&mut buf));
        String::from_utf8(buf).unwrap()
    }
}

pub struct HtmlIter<B: BufRead> {
    reader: quick_xml::Reader<B>,
    read_buf: Vec<u8>,
}

impl<B: BufRead> HtmlIter<B> {
    pub fn from_reader(reader: B) -> Self {
        Self {
            reader: quick_xml::Reader::from_reader(reader),
            read_buf: Vec::new(),
        }
    }
}

impl<B: io::BufRead> HtmlIterator for HtmlIter<B> {
    fn next(&mut self) -> Option<quick_xml::events::Event> {
        self.read_buf.clear();
        match self.reader.read_event(&mut self.read_buf) {
            Ok(quick_xml::events::Event::Eof) => None,
            Ok(e) => Some(e),
            Err(e) => panic!("{}", e),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn doc_identity() {
        let test = "<!DOCTYPE html><html><head></head><body><p><b>hello</b></p><p>world!</p></body></html>";
        let out = HtmlIter::from_reader(test.as_bytes());
        assert_eq!(&out.to_string(), test);
    }

    #[test]
    fn fragment_identity() {
        let test = "<p><b>hello</b></p><p>world!</p>";
        let out = HtmlIter::from_reader(test.as_bytes());
        assert_eq!(&out.to_string(), test);
    }

    // #[test]
    // fn remove_elements() {
    //     let test = r#"<!DOCTYPE html><html><head></head><body><p class="hello"><b>hello</b></p><p>world!</p></body></html>"#;
    //     let out = HtmlIter::from_read(test.as_bytes()).exclude(css_select!(."hello"));
    //     assert_eq!(
    //         &out.to_string(),
    //         r#"<!DOCTYPE html><html><head></head><body><p>world!</p></body></html>"#
    //     );
    // }

    // #[test]
    // fn select_element() {
    //     let test = "<!DOCTYPE html><html><head></head><body><div id="main"><p><b>hello</b></p><p>world!</p></div><p>side</p></body></html>";
    //     let out = HtmlIter::from_read(test.as_bytes()).include(css_select!((#"main" "p"));
    //     assert_eq!(&out.to_string(), "<p><b>hello</b></p><p>world!</p>");
    // }
}
