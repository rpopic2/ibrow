use crossterm::cursor;
use crossterm::event::{poll, read, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use input::*;
use page::*;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, Read};
use std::process::Command;
use std::time::Duration;

mod input;
mod page;
mod pager;

fn main() -> std::io::Result<()> {
    let mut stdout = std::io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let mut screen_size: (u16, u16) = terminal::size().unwrap();

    let mut cur_url = String::new();
    let mut cur_line = 0u16;
    let mut bookmark = String::new();
    let mut cur_page: Page = Page {
        buf: String::new(),
        line_count: 0,
        anchors: Vec::new(),
    };

    if let Ok(mut f) = File::open("ibrow.conf") {
        f.read_to_string(&mut bookmark).unwrap();
    }

    enable_raw_mode()?;
    loop {
        if poll(Duration::from_millis(1000))? {
            let ev = read()?;
            let ev = match ev {
                Event::Key(k) => k,
                Event::Resize(w, h) => {
                    screen_size.0 = w;
                    screen_size.1 = h;
                    continue;
                }
                _ => continue,
            };
            if ev.modifiers == KeyModifiers::CONTROL {
                match ev.code {
                    KeyCode::Char('c') => break,
                    KeyCode::Char('e') => {
                        cur_line = cur_line
                            .saturating_add(1)
                            .clamp(0, (cur_page.line_count as u16) - 1);
                        pager::pager(&cur_page.buf, cur_line)?;
                    }
                    KeyCode::Char('y') => {
                        cur_line = cur_line.saturating_sub(1);
                        pager::pager(&cur_page.buf, cur_line)?;
                    }
                    KeyCode::Char('f') => {
                        cur_line = cur_line
                            .saturating_add(screen_size.1 / 2)
                            .clamp(0, cur_page.line_count as u16 - 1);
                        pager::pager(&cur_page.buf, cur_line)?;
                    }
                    KeyCode::Char('b') => {
                        cur_line = cur_line.saturating_sub(screen_size.1 / 2);
                        pager::pager(&cur_page.buf, cur_line)?;
                    }
                    _ => (),
                }
            } else if ev.modifiers == KeyModifiers::NONE {
                match ev.code {
                    KeyCode::Char('f') => {
                        let path = match get_input("files: ") {
                            Ok(s) => s,
                            Err(_) => continue,
                        };
                        match File::open(path) {
                            Ok(mut file) => {
                                let mut buf = String::new();
                                file.read_to_string(&mut buf)?;
                                cur_page = get_processed_page(&buf);
                                pager::pager(&cur_page.buf, 0)?;
                            }
                            Err(_) => {
                                println!("could not find file");
                            }
                        }
                        stdout.execute(cursor::MoveToColumn(0))?;
                    }
                    KeyCode::Char('g') => {
                        let url = match get_input("goto: ") {
                            Ok(s) => s,
                            Err(_) => continue,
                        };
                        cur_url = url;
                        let buf = go_url(&cur_url)?;
                        cur_page = get_processed_page(&buf);
                        pager::pager(&cur_page.buf, 0)?;
                        stdout.execute(cursor::MoveToColumn(0))?;
                    }
                    KeyCode::Char('d') => {
                        let Ok(data) = get_input("data: ") else {
                            continue;
                        };
                        let buf = post(&cur_url, &data)?;
                        cur_page = get_processed_page(&buf);
                        pager::pager(&cur_page.buf, 0)?;
                        stdout.execute(cursor::MoveToColumn(0))?;
                    }
                    KeyCode::Char('m') => {
                        bookmark = match get_input_with("bookmark: ", Some(&cur_url)) {
                            Ok(s) => s,
                            Err(_) => continue,
                        }
                    }
                    KeyCode::Char('`') => {
                        cur_url = match get_input_with("goto: ", Some(&bookmark)) {
                            Ok(s) => s,
                            Err(_) => continue,
                        };
                        let buf = go_url(&cur_url)?;
                        let page = get_processed_page(&buf);
                        pager::pager(&page.buf, 0)?;
                        stdout.execute(cursor::MoveToColumn(0))?;
                    }
                    KeyCode::Char('a') => {
                        let Ok(s) = get_input("anchor(index): ") else {
                            continue;
                        };
                        let Ok(index) = s.parse::<usize>() else {
                            continue;
                        };
                        let Some(url) = cur_page.anchors.get(index) else {
                            continue;
                        };
                        let url = if url.starts_with('/') {
                            let mut mod_url = url.to_owned();
                            mod_url.insert_str(0, &cur_url);
                            mod_url
                        } else {
                            url.to_string()
                        };
                        let buf = go_url(&url)?;
                        cur_page = get_processed_page(&buf);
                        pager::pager(&cur_page.buf, 0)?;
                        stdout.execute(cursor::MoveToColumn(0))?;
                    }
                    _ => (),
                }
            } else {
                match ev.code {
                    KeyCode::Char('G') => {
                        cur_url = match get_input_with("goto: ", Some(&cur_url)) {
                            Ok(s) => s,
                            Err(_) => continue,
                        };
                        let buf = go_url(&cur_url)?;
                        cur_page = get_processed_page(&buf);
                        pager::pager(&cur_page.buf, 0)?;
                        stdout.execute(cursor::MoveToColumn(0))?;
                    }
                    _ => (),
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

fn go_url(url: &String) -> io::Result<String> {
    println!("{}", url);
    curl(["-#", "-L", &url])
}

fn post(url: &String, data: &str) -> io::Result<String> {
    println!("{}, {}", url, data);
    curl(["-#", "-d", data, "-L", &url])
}

fn curl<I, S>(args: I) -> io::Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut stdout = io::stdout();
    stdout.execute(cursor::MoveTo(0, 0))?;
    stdout.execute(Clear(ClearType::All))?;
    disable_raw_mode()?;

    let mut curl = Command::new("curl");
    curl.args(args);
    let curl_out = curl.output().expect("curl error");
    println!("{}", std::str::from_utf8(&curl_out.stderr).unwrap());
    let curl_out = String::from_utf8(curl_out.stdout).expect("utf8 error");

    enable_raw_mode()?;
    Ok(curl_out)
}
