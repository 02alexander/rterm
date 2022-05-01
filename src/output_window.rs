
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
    data: String,
    pub window: Window,
}

impl OutputWindow {
    pub fn new(window: Window) -> Self {
        OutputWindow {
            data: String::new(),
            window,
        }
    }

    pub fn add_data(&mut self, s: &str) {
        self.window.addnstr(&s, s.len());
        self.data.push_str(s);
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
