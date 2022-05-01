
use pancurses::*;
use super::termdev::TerminalDevice;
use std::io::Write;

pub struct InputWindow {
    history: Vec<String>,
    cur_line: String,
    window: Window,
}

impl InputWindow {
    pub fn new(width: i32, start_y: i32) -> Self {
        let window = newwin(3, width, start_y,0);
        let hline = '-';
        window.border(' ', ' ', hline,hline,hline,hline,hline,hline);
        window.timeout(1);
        window.keypad(true);
        window.mv(1, 0);
        InputWindow {
            history: Vec::new(),
            cur_line: String::new(),
            window,
        }
    }

    pub fn update(&mut self, td: &mut TerminalDevice) -> anyhow::Result<Option<pancurses::Input>> {
        if let Some(inp) = self.window.getch() {
            match inp {
                pancurses::Input::Character(ch) => {
                    self.cur_line.push(ch);
                    if ch == '\n' {
                        self.history.push(self.cur_line.clone());
                        td.write(&self.cur_line.as_bytes())?;
                        self.window.mv(1,0);
                        self.window.clrtoeol();
                        self.cur_line.clear();
                    }
                },
                _ => {
                    return Ok(Some(inp));
                }
            }
        }
        Ok(None)
    }
}