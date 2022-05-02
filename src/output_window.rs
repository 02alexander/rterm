
use pancurses::Window;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::Receiver;
use lazy_static::lazy_static;
use super::termdev::TerminalDevice;
use std::io::Read;

lazy_static! {
    static ref SCREEN_LOCK: Mutex<()> = Mutex::new(());
}

pub struct OutputWindow {

    // Content of all lines received from arduino. Excludes '\n'.
    lines: Vec<String>,
    
    pub window: Window,
}

impl OutputWindow {
    pub fn new(window: Window) -> Self {
        OutputWindow {
            lines: vec![String::new()],
            window,
        }
    }

    pub fn reprint_lines(&mut self) {
        self.window.mv(0, 0);
        let height = self.window.get_max_y();
        let len = self.lines.len();
        for line in &self.lines[0i32.max(len as i32-height-1) as usize..] {
            self.window.clrtoeol();
            self.window.addstr(line);
            self.window.addch('\n');
        }
    }

    pub fn add_data(&mut self, s: &str) {
        let height = self.window.get_max_y();
        for ch in s.chars() {
            if ch == '\n' {
                let cur_y = self.window.get_cur_y();
                self.lines.push(String::new());
                if cur_y == height-1 {
                    self.reprint_lines();
                }
            } else {
                self.lines.last_mut().unwrap().push(ch);
            }
            self.window.addch(ch);

        }
        self.window.refresh();
    }
    
    pub fn update(&mut self, td: &mut TerminalDevice) {
        let mut buf = [0 as u8; 256];
        if let Ok(n) = td.read(&mut buf) {
            if let Ok(s) = String::from_utf8(buf[0..n].to_vec()) {
                self.add_data(&s);
            }
        }
    }
}

unsafe impl Send for OutputWindow {}
