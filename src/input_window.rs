use super::termdev::TerminalDevice;
use pancurses::*;
use std::io::Write;

pub struct InputWindow {
    pub history: Vec<String>,
    cur_hist_idx: Option<usize>,
    pub cur_line: String,
    pub window: Window,
}

impl InputWindow {
    pub fn new(width: i32, start_y: i32) -> Self {
        let window = newwin(3, width, start_y, 0);
        let hline = '-';
        window.border(' ', ' ', hline, hline, hline, hline, hline, hline);
        window.timeout(1);
        window.keypad(true);
        window.mv(1, 0);
        InputWindow {
            history: Vec::new(),
            cur_hist_idx: None,
            cur_line: String::new(),
            window,
        }
    }

    fn clear_line(&mut self) {
        self.window.mv(1, 0);
        self.window.clrtoeol();
        self.cur_line.clear();
    }

    fn advance_in_hist(&mut self, step: i32) {
        self.cur_hist_idx = match self.cur_hist_idx {
            Some(ref mut idx) => {
                let next_idx = *idx as i32 + step;
                if next_idx <= 0 {
                    Some(0)
                } else if next_idx >= self.history.len() as i32 {
                    self.cur_line.clear();
                    None
                } else {
                    Some(next_idx as usize)
                }
            }
            None => {
                if self.history.len() == 0 {
                    None
                } else if step < 0 {
                    Some(self.history.len() - 1)
                } else {
                    None
                }
            }
        };
        if let Some(idx) = self.cur_hist_idx {
            self.cur_line = self.history[idx].clone();
        }
        self.update_input();
    }

    // Updates the input window with self.cur_line.
    fn update_input(&mut self) {
        self.window.mv(1, 0);
        self.window.clrtoeol();
        self.window.addstr(self.cur_line.clone());
        self.window.refresh();
    }

    fn delete_ch(&mut self) {
        self.cur_line.pop();
        self.update_input();
    }

    // Gets character input from user and handles this character. If the function doesn't know what to do
    // with a character the it returns it.
    pub fn update(&mut self, td: &mut TerminalDevice) -> anyhow::Result<Option<pancurses::Input>> {
        if let Some(inp) = self.window.getch() {
            match inp {
                pancurses::Input::Character(ch) => {
                    if ch as u8 == 127 {
                        self.delete_ch();
                    } else {
                        self.cur_hist_idx = None;
                        self.cur_line.push(ch);
                        if ch == '\n' {
                            // Adds string to history (excluding '\n').
                            self.history
                                .push(self.cur_line[0..self.cur_line.len() - 1].to_owned());
                            td.write(&self.cur_line.as_bytes())?;
                            self.clear_line();
                        }
                        self.update_input();
                    }
                }
                pancurses::Input::KeyUp => {
                    self.advance_in_hist(-1);
                }
                pancurses::Input::KeyDown => {
                    self.advance_in_hist(1);
                }
                pancurses::Input::KeyBackspace => {
                    self.delete_ch();
                }
                _ => {
                    return Ok(Some(inp));
                }
            }
        }
        Ok(None)
    }
}
