use std::fmt::Write;
use crossterm::style::Stylize;

pub struct ProcessedPage {
    pub buf: String,
    pub line_count: usize,
    pub anchors: Vec<String>,
}

pub fn get_processed_page(page: &str) -> ProcessedPage {
    let curl_out = page.replace("&nbsp;", "\u{A0}");
    let curl_out = curl_out.replace("&quot;", "\"");
    let mut iter = curl_out.split(|c| c == '<');

    let mut anchors: Vec<String> = Vec::new();

    let mut input_group = false;
    let mut buf = String::new();
    loop {
        if let Some(i) = iter.next() {
            let cur_input_group = i.starts_with("input");
            if input_group && !cur_input_group {
                writeln!(&mut buf).unwrap();
            }
            input_group = cur_input_group;
            write_elem(i, &mut buf, &mut anchors);
        } else {
            if input_group { writeln!(&mut buf).unwrap(); }
            break;
        }
    }
    let count = buf.lines().count();
    ProcessedPage{ buf, line_count: count, anchors }
}

fn write_elem(s: &str, buf: &mut String, anchors: &mut Vec<String>) {
    if s.starts_with("br>") {
        writeln!(buf).unwrap();
    } else if s.starts_with("p>") {
        writeln!(buf).unwrap();
    } else if s.starts_with("/p>") {
        print_rest(s, buf);
        writeln!(buf).unwrap();
    } else if s.starts_with("input") {
        if let Some(name) = get_attr(s, "name") {
            write!(buf, "__{}", name).unwrap();
            if let Some(attr) = get_attr(s, "value") {
                write!(buf, "={}", attr).unwrap();
            }
            write!(buf, "__").unwrap();
        }
        print_rest(s, buf);
    } else if s.starts_with("b>") {
        if let Some(tag_end) = s.find('>') {
            let text = s.get(tag_end + 1..).unwrap();
            if !text.is_empty() {
                write!(buf, "{}", text.bold()).unwrap();
            }
        } else {
            write!(buf, "{}", s.bold()).unwrap();
        }
    } else if s.starts_with("script") || s.starts_with("style") {

    } else if s.starts_with("option") {

    } else if s.starts_with("tr") {
        writeln!(buf).unwrap();
    } else if s.starts_with("a ") {
        if let Some(a) = get_attr(s, "href") {
            write!(buf, "[{}: ", anchors.len()).unwrap();
            print_rest(s, buf);
            write!(buf, "]").unwrap();
            write!(buf, "(").unwrap();
            write!(buf, "{}", a).unwrap();
            write!(buf, ")").unwrap();
            anchors.push(a.to_owned());
        }
    } else {
        print_rest(s, buf);
    }
}

fn print_rest(s: &str, buf: &mut String) {
    if let Some(tag_end) = s.find('>') {
        let tag_end = tag_end + 1;
        let text = s.get(tag_end..).unwrap()
            .split_terminator(&['\n', '\r']);
        for s in text {
            write!(buf, "{}", s).unwrap();
        }
    } else {
        write!(buf, "{}", s).unwrap();
    }
}

fn is_quote(c: char) -> bool {
    c == '"' || c == '\''
}

fn is_space(c: char) -> bool {
    c == ' '
}
type Pred = fn(c: char) -> bool;

fn get_attr<'a>(input: &'a str, attr: &'a str) -> Option<&'a str> {
    if let Some(idx) = input.find(attr) {
        let s1 = input.get(idx..).unwrap();
        let first_quote = s1.find(is_quote);
        let (first_quote, next_query): (usize, Pred) = match first_quote {
            Some(_) => (first_quote.unwrap() + 1, is_quote),
            None => (0usize, is_space)
        };
        let s2 = s1.get(first_quote..).unwrap();
        let second_quote = s2.find(next_query).unwrap();
        Some(s2.get(..second_quote).unwrap())
    } else {
        None
    }
}
