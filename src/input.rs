use std::io::{self, Stdout, Error};
use std::process::Command;
use std::time::Duration;

use crossterm::event::{poll, read, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{Clear, ClearType};
use crossterm::{ExecutableCommand, QueueableCommand};
use crossterm::cursor;

pub fn get_input(prompt: &str) -> io::Result<String> {
    get_input_with(prompt, None)
}

pub fn get_input_with(prompt: &str, start_val: Option<&str>) -> io::Result<String> {
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
            let Event::Key(e) = read()? else { continue };
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
            } else if e.modifiers == KeyModifiers::CONTROL || e.modifiers ==  KeyModifiers::SHIFT {
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
                        buf = buf.get(..cursor_pos()? as usize).unwrap().to_string();
                        println!("buf: {}", buf);
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

    stdout.execute(cursor::RestorePosition)?;

    let buf = String::from(buf.trim());
    if buf.is_empty() {
        return Err(Error::new(io::ErrorKind::InvalidInput, "aborted"));
    }
    Ok(buf)
}

