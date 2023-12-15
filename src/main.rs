use std::fs::File;
use std::io::{self, prelude::*, Error, Stdout};
use std::fmt::Write;
use std::time::Duration;
use std::process::Command;
use colored::Colorize;
use crossterm::event::{poll, read, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{Clear, ClearType, self};
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use crossterm::{ExecutableCommand, QueueableCommand};
use crossterm::cursor;

fn main() -> std::io::Result<()> {
    let mut stdout = std::io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let screen_size = terminal::size()?;

    let mut cur_url = String::new();
    let mut cur_buf = String::new();
    let mut cur_line = 0u16;
    let mut buf_line_len = 0usize;
    let mut bookmark = String::new();

    if let Ok(mut f) = File::open("ibrow.conf") {
        f.read_to_string(&mut bookmark).unwrap();
    }

    enable_raw_mode()?;
    loop {
        if poll(Duration::from_millis(1000))? {
            if let Event::Key(ev) = read()? {
                if ev.modifiers == KeyModifiers::CONTROL {
                    match ev.code {
                        KeyCode::Char('c') => break,

                        KeyCode::Char('e') => {
                            cur_line = cur_line.saturating_add(1).clamp(0, (buf_line_len as u16) - 1);
                            pager(&cur_buf, cur_line, screen_size)?;
                        }
                        KeyCode::Char('y') => {
                            cur_line = cur_line.saturating_sub(1);
                            pager(&cur_buf, cur_line, screen_size)?;
                        }
                        KeyCode::Char('f') => {
                            cur_line = cur_line.saturating_add(screen_size.1 / 2).clamp(0, buf_line_len as u16 - 1);
                            pager(&cur_buf, cur_line, screen_size)?;
                        }
                        KeyCode::Char('b') => {
                            cur_line = cur_line.saturating_sub(screen_size.1 / 2);
                            pager(&cur_buf, cur_line, screen_size)?;
                        }
                        _ => ()
                    }
                } else if ev.modifiers == KeyModifiers::NONE {
                    match ev.code {
                        KeyCode::Char('f') => {
                            let path = match get_input("files: ") {
                                Ok(s) => s,
                                Err(_) => continue
                            };
                            match File::open(path) {
                                Ok(mut file) => {
                                    let mut buf = String::new();
                                    file.read_to_string(&mut buf)?;
                                    (cur_buf, buf_line_len) = get_processed_page(&buf);
                                    pager(&cur_buf, 0, screen_size)?;
                                    stdout.execute(cursor::MoveToColumn(0))?;
                                }
                                Err(_) => {
                                    println!("could not find file");
                                    stdout.execute(cursor::MoveToColumn(0))?;
                                }
                            }
                        }
                        KeyCode::Char('g') => {
                            let url = match get_input("goto: ") {
                                Ok(s) => s,
                                Err(_) => continue
                            };
                            cur_url = url;
                            cur_buf = go_url(&cur_url)?;
                            (cur_buf, buf_line_len) = get_processed_page(&cur_buf);
                            pager(&cur_buf, 0, screen_size)?;
                            stdout.execute(cursor::MoveToColumn(0))?;
                        }
                        KeyCode::Char('d') => {
                            let data = match get_input("data: ") {
                                Ok(s) => s,
                                Err(_) => continue
                            };
                            cur_buf = post(&cur_url, &data)?;
                            (cur_buf, buf_line_len) = get_processed_page(&cur_buf);
                            pager(&cur_buf, 0, screen_size)?;
                            stdout.execute(cursor::MoveToColumn(0))?;
                        }
                        KeyCode::Char('m') => {
                            bookmark = match get_input_with("bookmark: ", Some(&cur_url)) {
                                Ok(s) => s,
                                Err(_) => continue
                            }
                        }
                        KeyCode::Char('`') => {
                            cur_url = match get_input_with("goto: ", Some(&bookmark)) {
                                Ok(s) => s,
                                Err(_) => continue
                            };
                            cur_buf = go_url(&cur_url)?;
                            (cur_buf, buf_line_len) = get_processed_page(&cur_buf);
                            pager(&cur_buf, 0, screen_size)?;
                            stdout.execute(cursor::MoveToColumn(0))?;
                        }
                    _ => ()
                    }
                } else {
                    match ev.code {
                        KeyCode::Char('G') => {
                            cur_url = match get_input_with("goto: ", Some(&cur_url)) {
                                Ok(s) => s,
                                Err(_) => continue
                            };
                            cur_buf = go_url(&cur_url)?;
                            (cur_buf, buf_line_len) = get_processed_page(&cur_buf);
                            pager(&cur_buf, 0, screen_size)?;
                            stdout.execute(cursor::MoveToColumn(0))?;
                        }
                        _ => ()
                    }
                }
            }
        }
    }
    disable_raw_mode()?;

    let mut bm = File::create("ibrow.conf").expect("failed to create config file");
    use std::io::Write;
    write!(&mut bm, "{}", bookmark).expect("failed to write config file");

    stdout.execute(LeaveAlternateScreen)?;
    Ok(())
}

fn pager(buf: &String, line: u16, screen_size: (u16, u16)) -> io::Result<()> {
    let mut stdout = io::stdout();
    // stdout.execute(cursor::MoveTo(offset.0, offset.1))?;
    stdout.execute(Clear(ClearType::FromCursorDown))?;
    stdout.execute(cursor::SavePosition)?;
    disable_raw_mode()?;

    let screen_width: usize = screen_size.0.into();
    let screen_height: usize = screen_size.1.into();
    let mut lines = buf.lines().skip(line.into());
    let mut counter = 0usize;
    while counter < screen_height {
        if let Some(s) = lines.next() {
            let wraps = s.len() / screen_width + 1;
            println!("{}", s);
            counter += wraps;
        } else {
            break;
        }
    }

    stdout.execute(cursor::RestorePosition)?;
    enable_raw_mode()?;
    Ok(())
}

fn get_input(prompt: &str) -> io::Result<String> {
    get_input_with(prompt, None)
}

fn get_input_with(prompt: &str, start_val: Option<&str>) -> io::Result<String> {
    let mut stdout = io::stdout();
    stdout.execute(cursor::SavePosition)?;
    stdout.execute(cursor::MoveTo(0, 0))?;
    stdout.execute(Clear(ClearType::CurrentLine))?;

    print!("{}", prompt);
    let cursor_zero = cursor::position()?.0;
    let mut buf = match start_val {
        Some(val) => {
            print!("{}", val);
            String::from(val)
        }
        None => String::new()
    };
    io::Write::flush(&mut stdout)?;


    let cursor_pos = || -> io::Result<u16> {
        Ok(cursor::position()?.0 - cursor_zero)
    };

    let refresh_after_cursor = |buf: &mut String, stdout: &mut Stdout, pos: usize| -> io::Result<()> {
        stdout.execute(Clear(ClearType::UntilNewLine))?;
        let cursor_pos = cursor::position()?.0;
        print!("{}", buf.get(pos..).unwrap());
        stdout.execute(cursor::MoveToColumn(cursor_pos))?;
        Ok(())
    };

    let bs = |buf: &mut String, stdout: &mut Stdout| -> io::Result<()> {
        let cursor_pos = cursor::position()?.0;
        let remove_pos = cursor_pos - cursor_zero;
        if remove_pos <= 0 {
            return Ok(())
        }
        buf.remove((remove_pos - 1).into());
        stdout.execute(cursor::MoveLeft(1))?;
        print!(" ");
        stdout.execute(cursor::MoveLeft(1))?;
        stdout.execute(Clear(ClearType::UntilNewLine))?;
        let cursor_pos = cursor::position()?.0;
        print!("{}", buf.get((remove_pos - 1) as usize..).unwrap());
        io::Write::flush(stdout)?;
        stdout.execute(cursor::MoveToColumn(cursor_pos))?;
        Ok(())
    };

    let insert_char = |buf: &mut String, stdout: &mut Stdout, c: char| -> io::Result<()> {
        buf.insert(cursor_pos()?.into(), c);
        print!("{}", c);
        refresh_after_cursor(buf, stdout, cursor_pos()?.into())?;
        io::Write::flush(stdout)?;
        Ok(())
    };

    let cancel = |buf: &mut String, stdout: &mut Stdout| -> io::Result<()> {
        stdout.execute(Clear(ClearType::CurrentLine))?;
        buf.clear();
        Ok(())
    };

    loop {
        if poll(Duration::from_millis(1000))? {
            if let Event::Key(e) = read()? {
                if e.modifiers == KeyModifiers::NONE {
                    match e.code {
                        KeyCode::Char(c) => insert_char(&mut buf, &mut stdout, c)?,
                        KeyCode::Enter => break,
                        KeyCode::Backspace => {
                            bs(&mut buf, &mut stdout)?;
                        }
                        KeyCode::Esc => {
                            cancel(&mut buf, &mut stdout)?;
                            break;
                        }
                        KeyCode::Tab => {
                            let mut ls = Command::new("ls");
                            let dir = match buf.is_empty() {
                                true => ".",
                                false => &buf
                            };
                            ls.args(["-m", dir]);
                            let pos = cursor::position()?;
                            stdout.execute(cursor::MoveTo(0, pos.1 + 1))?;
                            ls.status().expect("ls failed");
                            stdout.execute(cursor::MoveTo(pos.0, pos.1))?;
                        }
                        _ => ()
                    }
                    continue;
                } else if e.modifiers == KeyModifiers::CONTROL {
                    match e.code {
                        KeyCode::Char('j') | KeyCode::Char('m') => break,
                        KeyCode::Char('c') => {
                            cancel(&mut buf, &mut stdout)?;
                            break;
                        }
                        KeyCode::Char('h') => {
                            bs(&mut buf, &mut stdout)?;
                        }
                        KeyCode::Char('u') => {
                            buf.clear();
                            stdout.queue(cursor::MoveToColumn(cursor_zero))?;
                            stdout.execute(Clear(ClearType::UntilNewLine))?;
                        }
                        KeyCode::Char('b') => {
                            if cursor_pos()? > 0 {
                                stdout.execute(cursor::MoveLeft(1))?;
                            }
                        }
                        KeyCode::Char('f') => {
                            if (cursor_pos()? as usize) < buf.len() {
                                stdout.execute(cursor::MoveRight(1))?;
                            }
                        }
                        KeyCode::Char('a') => {
                            stdout.execute(cursor::MoveToColumn(cursor_zero))?;
                        }
                        KeyCode::Char('e') => {
                            let pos = cursor_zero + buf.len() as u16;
                            stdout.execute(cursor::MoveToColumn(pos))?;
                        }
                        _ => ()
                    }
                } else if let KeyCode::Char(c) = e.code {
                    insert_char(&mut buf, &mut stdout, c)?;
                }
            }
        }
    }

    stdout.execute(cursor::RestorePosition)?;

    let buf = String::from(buf.trim());
    if buf.is_empty() {
        return Err(Error::new(io::ErrorKind::InvalidInput, "aborted"));
    }
    Ok(buf)
}

fn go_url(url: &String) -> io::Result<String> {
    let mut stdout = io::stdout();
    stdout.execute(cursor::MoveTo(0, 0))?;
    stdout.execute(Clear(ClearType::All))?;
    disable_raw_mode()?;
    println!("{}", url);
    let mut curl = Command::new("curl");
    curl.args(["-#", "-L", &url]);

    let curl_out = curl.output().expect("curl error");
    if !curl_out.status.success() {
        println!("curl error occured: {}", curl_out.status);
    }
    println!("{}", std::str::from_utf8(&curl_out.stderr).unwrap());
    let curl_out = std::str::from_utf8(&curl_out.stdout).expect("utf8 error");


    enable_raw_mode()?;
    Ok(curl_out.to_string())
}

fn post(url: &String, data: &str) -> io::Result<String> {
    let mut stdout = io::stdout();
    stdout.execute(cursor::MoveTo(0, 0))?;
    stdout.execute(Clear(ClearType::All))?;
    disable_raw_mode()?;

    println!("{}, {}", url, data);
    let mut curl = Command::new("curl");
    curl.args(["-#", "-d", data, "-L", &url]);

    let curl_out = curl.output().expect("curl error");
    if !curl_out.status.success() {
        println!("curl error occured: {}", curl_out.status);
    }
    println!("{}", std::str::from_utf8(&curl_out.stderr).unwrap());
    let page = std::str::from_utf8(&curl_out.stdout).unwrap();

    enable_raw_mode()?;
    Ok(page.to_string())
}

fn get_processed_page(page: &str) -> (String, usize) {
    let curl_out = page.replace("&nbsp;", "\u{A0}");
    let curl_out = curl_out.replace("&quot;", "\"");
    let mut iter = curl_out.split(|c| c == '<');

    let mut input_group = false;
    let mut buf = String::new();
    loop {
        if let Some(i) = iter.next() {
            let cur_input_group = i.starts_with("input");
            if input_group && !cur_input_group {
                writeln!(&mut buf).unwrap();
            }
            input_group = cur_input_group;
            write_elem(i, &mut buf);
        } else {
            if input_group { writeln!(&mut buf).unwrap(); }
            break;
        }
    }
    let count = buf.lines().count();
    (buf, count)
}

fn write_elem(s: &str, buf: &mut String) {
    if s.starts_with("br>") {
        writeln!(buf).unwrap();
    } else if s.starts_with("p>") {
        writeln!(buf).unwrap();
    } else if s.starts_with("/p>") {
        print_rest(s, buf);
        writeln!(buf).unwrap();
    } else if s.starts_with("input") {
        if let Some(name) = get_attr(s, "name") {
            write!(buf, "{{{}", name).unwrap();
            if let Some(attr) = get_attr(s, "value") {
                write!(buf, "={}", attr).unwrap();
            }
            write!(buf, "}}").unwrap();
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
        write!(buf, "[").unwrap();
        print_rest(s, buf);
        write!(buf, "]").unwrap();
        if let Some(a) = get_attr(s, "href") {
            write!(buf, "(").unwrap();
            write!(buf, "{}", a).unwrap();
            write!(buf, ")").unwrap();
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

