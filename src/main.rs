extern crate pancurses;
extern crate libc;
extern crate clap;
extern crate lazy_static;

mod termdev;
mod output_window;
mod input_window;

use output_window::OutputWindow;
use input_window::InputWindow;
use termdev::TerminalDevice;
use pancurses::*;

fn main() {
    let mut td = TerminalDevice::new("/dev/ttyACM0").unwrap();
    td.configure_for_arduino(nix::sys::termios::BaudRate::B9600).unwrap();

    
    let screen = initscr();
    cbreak();
    noecho();
    let height = screen.get_max_y();
    let width = screen.get_cur_x();
    let window = newwin(height-5, width, 5, 0);
    let mut ow = OutputWindow::new(window);
    let mut iw = InputWindow::new(width, 2);
    loop {
        ow.update(&mut td);
        if let Some(special_ch) = iw.update(&mut td).unwrap() {
            ow.add_data(&format!("{:?}", special_ch));
        }
    }
    endwin();

}