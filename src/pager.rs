use std::io;

use crossterm::{
    cursor,
    terminal::{self, disable_raw_mode, enable_raw_mode, Clear, ClearType},
    ExecutableCommand, QueueableCommand,
};

pub fn pager(buf: &String, line: u16) -> io::Result<()> {
    let mut stdout = io::stdout();
    stdout.execute(cursor::SavePosition)?;
    stdout.execute(cursor::MoveToColumn(0))?;
    stdout.execute(cursor::MoveToRow(0))?;
    stdout.execute(Clear(ClearType::FromCursorDown))?;
    disable_raw_mode()?;

    let (screen_width, screen_height) = terminal::size()?;
    let screen_width: usize = screen_width.into();
    let screen_height: usize = screen_height.into();
    let mut lines = buf.lines().skip(line.into());
    let mut counter = 0usize;
    while counter < screen_height {
        if let Some(s) = lines.next() {
            let wraps = s.len() / screen_width + 1;
            print!("w: {}|", wraps);
            print!("{}", s);
            stdout.queue(cursor::MoveDown(1))?;
            stdout.queue(cursor::MoveToColumn(0))?;
            counter += wraps;
        } else {
            break;
        }
    }

    stdout.execute(cursor::RestorePosition)?;
    enable_raw_mode()?;
    Ok(())
}
