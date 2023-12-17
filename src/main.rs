use crossterm::cursor;
use crossterm::event::{poll, read, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use input::*;
use page::*;
use history::*;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, Read};
use std::process::Command;
use std::time::Duration;

mod input;
mod page;
mod pager;
mod history;

fn main() -> std::io::Result<()> {
    let mut stdout = std::io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let mut screen_size: (u16, u16) = terminal::size().unwrap();

    let mut cur_line = 0u16;
    let mut history: History = History::new();

    let mut bookmark_path = home::home_dir().unwrap();
    bookmark_path.push("ibrow.conf");
    let bookmark_path = bookmark_path.into_os_string();
    let mut bookmark = String::new();
    if let Ok(mut f) = File::open(&bookmark_path) {
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
                            .clamp(0, (history.current().line_count as u16) - 1);
                        pager::pager(&history.current().buf, cur_line)?;
                        // stdout.execute(terminal::ScrollUp(1))?;
                    }
                    KeyCode::Char('y') => {
                        cur_line = cur_line.saturating_sub(1);
                        pager::pager(&history.current().buf, cur_line)?;
                        // stdout.execute(terminal::ScrollDown(1))?;
                    }
                    KeyCode::Char('f') => {
                        cur_line = cur_line
                            .saturating_add(screen_size.1 / 2)
                            .clamp(0, history.current().line_count as u16 - 1);
                        pager::pager(&history.current().buf, cur_line)?;
                    }
                    KeyCode::Char('b') => {
                        cur_line = cur_line.saturating_sub(screen_size.1 / 2);
                        pager::pager(&history.current().buf, cur_line)?;
                    }
                    KeyCode::Char('o') => {
                        history.prev();
                        pager::pager(&history.current().buf, 0).unwrap();
                    }
                    _ => (),
                }
            } else if ev.modifiers == KeyModifiers::NONE {
                match ev.code {
                    KeyCode::Tab => {
                        history.next();
                        pager::pager(&history.current().buf, 0).unwrap();
                    }
                    KeyCode::Char('f') => {
                        let path = match get_input("files: ") {
                            Ok(s) => s,
                            Err(_) => continue,
                        };
                        match File::open(path) {
                            Ok(mut file) => {
                                let mut buf = String::new();
                                file.read_to_string(&mut buf)?;
                                history.push(get_processed_page(&buf));
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
                        let buf = go_url(&url)?;
                        history.push(get_processed_page(&buf));
                    }
                    KeyCode::Char('d') => {
                        let Ok(data) = get_input("data: ") else {
                            continue;
                        };
                        let buf = post(&history.current().url, &data)?;
                        history.push(get_processed_page(&buf));
                    }
                    KeyCode::Char('`') => {
                        let url = match get_input_with("goto: ", Some(&bookmark)) {
                            Ok(s) => s,
                            Err(_) => continue,
                        };
                        let buf = go_url(&url)?;
                        history.push(get_processed_page(&buf));
                    }
                    KeyCode::Char('a') => {
                        let Ok(s) = get_input("anchor index: ") else {
                            continue;
                        };
                        let Ok(index) = s.parse::<usize>() else {
                            continue;
                        };
                        let Some(url) = history.current().anchors.get(index) else {
                            continue;
                        };
                        let url = if url.starts_with('/') {
                            let mut mod_url = url.to_owned();
                            mod_url.insert_str(0, &history.current().url);
                            mod_url
                        } else {
                            url.to_string()
                        };
                        let buf = go_url(&url)?;
                        history.push(get_processed_page(&buf));
                    }
                    _ => (),
                }
            } else {
                match ev.code {
                    KeyCode::Char('G') => {
                        let url = match get_input_with("goto: ", Some(&history.current().url)) {
                            Ok(s) => s,
                            Err(_) => continue,
                        };
                        let buf = go_url(&url)?;
                        history.push(get_processed_page(&buf));
                    }
                    _ => (),
                }
            }
        }
    }
    disable_raw_mode()?;

    let mut bm = File::create(bookmark_path).expect("failed to create config file");
    use std::io::Write;
    write!(&mut bm, "{}", bookmark).expect("failed to write config file");

    stdout.execute(LeaveAlternateScreen)?;
    Ok(())
}

fn go_url(url: &str) -> io::Result<String> {
    println!("{}", url);
    curl(["-#", "-w %{url_effective}", "-L", url])
}

fn post(url: &str, data: &str) -> io::Result<String> {
    println!("{}, {}", url, data);
    curl(["-#", "-w %{url_effective}", "-L", url, "-d", data])
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
