mod app;
mod termdev;

use std::{panic::{self, AssertUnwindSafe}};

use anyhow::{anyhow, Context};
use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use nix::sys::termios::BaudRate;
use regex::Regex;
use termdev::TerminalDevice;
use tui::{backend::CrosstermBackend, Terminal};

use crate::app::Grapher;

#[derive(Parser)]
#[clap(author, version, about, long_about=None)]
struct Cli {
    #[clap(short, long, default_value_t = 9600)]
    baudrate: u32,

    #[clap(short, long)]
    terminal_device: Option<String>,

    #[clap(short, long)]
    out_file: Option<String>,

    #[clap(short, long)]
    graph: bool,

    #[clap(long, default_value_t=60)]
    graph_len: usize 
}

fn find_possible_arduino_dev() -> Option<String> {
    for dir_entry in std::fs::read_dir("/dev/").ok()? {
        let dir_entry = dir_entry.ok()?;
        let os_file_name = dir_entry.file_name();
        let file_name = os_file_name.to_string_lossy();
        if file_name.starts_with("tty") {
            if file_name.len() >= 6 {
                if &file_name[3..6] == "USB" || &file_name[3..6] == "ACM" {
                    return Some("/dev/".to_string() + &file_name);
                }
            }
        }
    }
    None
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

fn main() -> anyhow::Result<()> {
    let parser = Cli::parse();

    let baudrate =
        string_to_baudrate(&format!("{}", parser.baudrate)).ok_or(anyhow!("invaild baubrate"))?;
    let tty_filepath = if let Some(path) = parser.terminal_device {
        path
    } else {
        find_possible_arduino_dev().ok_or(anyhow!(
            "Could not find any open serial port automatically, please specify port"
        ))?
    };

    let out_filepath = parser.out_file;

    let outfile = if let Some(fname) = out_filepath {
        Some(std::fs::File::create(&fname).context(format!("opening '{}'", &fname))?)
    } else {
        None
    };

    let mut td =
        TerminalDevice::new(tty_filepath.clone()).context(format!("opening '{tty_filepath}'"))?;
    td.configure_for_arduino(baudrate)?;

    let mut app = app::App::new(outfile);
    if parser.graph {
        app.grapher = Some(Grapher {
            data: Vec::new(),
            value_pattern: Regex::new("(\\-?\\d+\\.?[\\d]*)").unwrap(),
            window_len: parser.graph_len,
            window: [0.0, parser.graph_len as f64]   
        });
    }

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Spawn main app in seperate thread so that the cleanup runs even when the app panics.
    let res = panic::catch_unwind(AssertUnwindSafe(|| {
        app.run(td, &mut terminal)
    }));

    // Cleanup.
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
    )?;
    disable_raw_mode()?;
    terminal.show_cursor()?;

    match res {
        Ok(res) => {
            println!("{:?}", res);
        },
        Err(e) => {
            println!("panicked: {:?}", e.downcast_ref::<&'static str>().unwrap());
        }
    }
    Ok(())
}
