use std::{
    fs::File,
    io::{self, Read, Write},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

use crossterm::event::{self, Event, KeyCode};
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame, Terminal,
};
use tui_textarea::TextArea;

use crate::termdev::TerminalDevice;

pub struct App {
    outfile: Option<File>,
    lines: Vec<String>,
    state: ListState,
    history: Vec<String>,
}

pub fn term_io_loop(
    td: TerminalDevice,
    stop: Receiver<()>,
    input: Receiver<Vec<u8>>,
    output: Sender<Vec<u8>>,
) -> anyhow::Result<()> {
    let (mut term_reader, mut term_writer) = td.split();

    let (read_thread_stop_tx, read_thread_stop_rx) = mpsc::channel();
    let (write_thread_stop_tx, write_thread_stop_rx) = mpsc::channel();

    // Reads from the terminal and sends the data to output.
    let term_reader_handle = thread::spawn(move || -> anyhow::Result<()> {
        loop {
            if let Ok(()) = read_thread_stop_rx.try_recv() {
                return Ok(());
            }
            let mut buf = vec![0; 8];
            match term_reader.read(&mut buf) {
                Ok(n) => {
                    if n != 0 {
                        output.send(Vec::from(&buf[..n]))?;
                    }
                }
                Err(e) => match e.kind() {
                    io::ErrorKind::WouldBlock => {}
                    _ => {}
                },
            }
        }
    });

    // Takes the data form input and reads if to the terminal device.
    let term_writer_handle = thread::spawn(move || -> anyhow::Result<()> {
        loop {
            if let Ok(()) = write_thread_stop_rx.try_recv() {
                return Ok(());
            }
            let data: Vec<u8> = input.recv()?;
            term_writer.write_all(&data)?;
            term_writer.flush()?;
        }
    });

    loop {
        match stop.try_recv() {
            Ok(()) => break,
            Err(mpsc::TryRecvError::Disconnected) => break,
            Err(mpsc::TryRecvError::Empty) => {}
        }
        if term_reader_handle.is_finished() || term_writer_handle.is_finished() {
            println!("a thread finished");
            break;
        }
    }
    let _ = read_thread_stop_tx.send(());
    let _ = write_thread_stop_tx.send(());
    let _ = term_reader_handle.join().unwrap()?;
    let _ = term_writer_handle.join().unwrap()?;
    Ok(())
}

impl App {
    pub fn new(outfile: Option<File>) -> Self {
        App {
            state: ListState::default(),
            outfile,
            lines: vec![String::new()],
            history: Vec::new(),
        }
    }

    pub fn run<B: Backend>(
        &mut self,
        td: TerminalDevice,
        terminal: &mut Terminal<B>,
    ) -> anyhow::Result<()> {
        let mut textarea = TextArea::default();

        let (stop_rx, stop_rc) = mpsc::channel();
        let (read_thread_tx, read_rx) = mpsc::channel();
        let (write_tx, write_thread_rx) = mpsc::channel();

        let _ = thread::spawn(|| term_io_loop(td, stop_rc, write_thread_rx, read_thread_tx));

        let res = 'event: loop {
            terminal.draw(|b| self.ui(b, &mut textarea))?;

            // Checke for any incoming bytes from the terminal device.
            if let Ok(res) = read_rx.try_recv() {
                for byte in &res {
                    if let Err(e) = self.parse_byte(*byte) {
                        break 'event Err(e);
                    };
                }
            }

            if let Ok(true) = event::poll(Duration::from_millis(1)) {
                if let Event::Key(key) = event::read()? {
                    if key.code == KeyCode::Esc {
                        return Ok(());
                    } else if key.code == KeyCode::Enter {
                        let mut line = textarea.lines()[0].clone();
                        textarea = TextArea::default();
                        line.push('\n');
                        write_tx.send(line.bytes().collect())?;
                        self.history.push(line);
                    } else {
                        textarea.input(key);
                    }
                }
            }
        };

        let _ = stop_rx.send(());

        res.map_err(|e| anyhow::anyhow!(e))
    }

    /// Parses a byte read from the terminal device.
    pub fn parse_byte(&mut self, byte: u8) -> std::io::Result<()> {
        if byte == 10 {
            if let Some(outfile) = &mut self.outfile {
                outfile.write_all(&mut format!("\n").into_bytes())?;
                outfile.flush()?;
            }
            self.lines.push(String::new());
        } else {
            let str = if let Ok(ch) = std::str::from_utf8(&[byte]) {
                format!("{}", ch.chars().next().unwrap())
            } else {
                // If it's not a vaild char, print out its hex value.
                format!("{byte:x}")
            };

            self.lines.last_mut().unwrap().push_str(&str);
            if let Some(outfile) = &mut self.outfile {
                outfile.write_all(&mut str.into_bytes())?;
                outfile.flush()?;
            }
        }
        Ok(())
    }

    /// Renders all the widgets ant their content.
    fn ui<B: Backend>(&mut self, f: &mut Frame<B>, textarea: &mut TextArea) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(4),
                Constraint::Length(1),
            ])
            .split(f.size());

        let input_block = Block::default().borders(Borders::ALL);
        textarea.set_block(input_block);
        f.render_widget(textarea.widget(), chunks[0]);

        let items: Vec<ListItem> = self
            .lines
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let row_number_style = Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::ITALIC);
                let text = Spans::from(vec![
                    Span::styled(format!("{:0>2}> ", i % 100), row_number_style),
                    Span::styled(line, Style::default()),
                ]);
                //text.extend(Span::raw(format!("'{}'", line));
                ListItem::new(text)
            })
            .collect();
        self.state.select(Some(items.len() - 1));
        let list = List::new(items);
        f.render_stateful_widget(list, chunks[1], &mut self.state);
    }
}
