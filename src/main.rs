extern crate pancurses;
extern crate clap;
extern crate lazy_static;
extern crate signal_hook;

mod termdev;
mod output_window;
mod input_window;

use signal_hook::{consts::SIGINT, iterator::Signals};
use output_window::OutputWindow;
use input_window::InputWindow;
use termdev::TerminalDevice;
use clap::Parser;
use nix::sys::termios::BaudRate;
use std::io::{Read, Write};
use pancurses::*;
use std::sync::atomic::{AtomicBool, Ordering};

static RUNNING: AtomicBool = AtomicBool::new(true);

#[derive(Parser)]
#[clap(author, version, about, long_about=None)]
struct Cli {

    #[clap(short, long, default_value_t=9600)]
    baudrate: u32,

    #[clap(short, long)]
    terminal_device: String,

    #[clap(short, long)]
    out_file: Option<String>
}

fn setup_signal_handler() -> anyhow::Result<()> {
    let mut signals = Signals::new(&[SIGINT])?;
    std::thread::spawn(move || {
        for sig in signals.forever() {
            if sig == signal_hook::consts::signal::SIGINT {
                while RUNNING.compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst).is_err() {
                    std::thread::yield_now();
                }
            }
        }
    });
    Ok(())
}

fn string_to_baudrate(s: &str) -> Option<BaudRate> {
    //baud_rate_comp!(s, 0, 50, 75, 110, 134, 150, 200, 300, 600, 1200, 1800, 2400, 4800, 9600, 19200, 38400, 57600, 115200, 230400, 460800, 500000, 576000, 921600, 1000000, 1152000, 1500000, 2000000, 2500000, 3000000, 3500000, 4000000)
    if s == "0" {
        Some(BaudRate::B0)
    } else if s == "50" {
        Some(BaudRate::B50)
    } else if s == "75" {
        Some(BaudRate::B75)
    } else if s == "110" {
        Some(BaudRate::B110)
    } else if s == "134" {
        Some(BaudRate::B134)
    } else if s == "150" {
        Some(BaudRate::B150)
    } else if s == "200" {
        Some(BaudRate::B200)
    } else if s == "300" {
        Some(BaudRate::B300)
    } else if s == "600" {
        Some(BaudRate::B600)
    } else if s == "1200" {
        Some(BaudRate::B1200)
    } else if s == "1800" {
        Some(BaudRate::B1800)
    } else if s == "2400" {
        Some(BaudRate::B2400)
    } else if s == "4800" {
        Some(BaudRate::B4800)
    } else if s == "9600" {
        Some(BaudRate::B9600)
    } else if s == "19200" {
        Some(BaudRate::B19200)
    } else if s == "38400" {
        Some(BaudRate::B38400)
    } else if s == "57600" {
        Some(BaudRate::B57600)
    } else if s == "115200" {
        Some(BaudRate::B115200)
    } else if s == "230400" {
        Some(BaudRate::B230400)
    } else if s == "460800" {
        Some(BaudRate::B460800)
    } else if s == "500000" {
        Some(BaudRate::B500000)
    } else if s == "576000" {
        Some(BaudRate::B576000)
    } else if s == "921600" {
        Some(BaudRate::B921600)
    } else if s == "1000000" {
        Some(BaudRate::B1000000)
    } else if s == "1152000" {
        Some(BaudRate::B1152000)
    } else if s == "1500000" {
        Some(BaudRate::B1500000)
    } else if s == "2000000" {
        Some(BaudRate::B2000000)
    } else if s == "2500000" {
        Some(BaudRate::B2500000)
    } else if s == "3000000" {
        Some(BaudRate::B3000000)
    } else if s == "3500000" {
        Some(BaudRate::B3500000)
    } else if s == "4000000" {
        Some(BaudRate::B4000000)
    } else {
        None
    }
}

fn main() {

    setup_signal_handler().unwrap();
    
    let parser = Cli::parse();

    let baudrate = match string_to_baudrate(&format!("{}", parser.baudrate)) {
        Some(brate) => brate,
        None => {
            println!("Error: '{}' is not a valid baudrate", parser.baudrate);
            return;
        }
    };

    let tty_filepath = parser.terminal_device;
    let out_filepath = parser.out_file;
    
    let mut outfile = if let Some(fname) = out_filepath {
        match std::fs::File::create(&fname) {
            Ok(f) => Some(f),
            Err(e) => {
                println!("Error opening {}: {}", &fname, e);
                return;
            }
        }
    } else {
        None
    };

    let mut td = match TerminalDevice::new(tty_filepath.clone()) {
        Ok(t) => t,
        Err(e) => {
            println!("Error opening {}: {}", &tty_filepath, e);
            return;
        }
    };
    td.configure_for_arduino(baudrate).unwrap();

    
    let screen = initscr();
    cbreak();
    noecho();
    curs_set(0);
    let height = screen.get_max_y();
    let width = screen.get_cur_x();
    let window = newwin(height-5, width, 5, 0);
    let mut ow = OutputWindow::new(window);
    let mut iw = InputWindow::new(width, 2);

    let mut buf = [0 as u8; 256];
    while RUNNING.load(Ordering::SeqCst) {
        if let Ok(n) = td.read(&mut buf) {
            if let Ok(s) = String::from_utf8(buf[0..n].to_vec()) {
                ow.add_data(&s);
                if let Some(ref mut f) = outfile {
                    let _ = f.write(&buf[0..n]);
                }
            }
        }

        if let Some(_special_ch) = iw.update(&mut td).unwrap() {
            //ow.add_data(&format!("{:?}", special_ch));
        }
    }
    endwin();
}