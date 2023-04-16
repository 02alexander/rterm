use std::{
    fs::File,
    io::{self, Read, Write},
    sync::mpsc::{self, Receiver, Sender},
    thread::{self},
    time::Duration,
};

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders},
    Frame, Terminal,
};
use tui_textarea::TextArea;

use crate::termdev::TerminalDevice;

pub struct App {
    outfile: Option<File>,
    history: Vec<String>,
}

pub struct UI {
    input_chunk: Rect,
    ouput_chunk: Rect,
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
            outfile,
            history: Vec::new(),
        }
    }

    pub fn run<B: Backend>(
        &mut self,
        td: TerminalDevice,
        terminal: &mut Terminal<B>,
    ) -> anyhow::Result<()> {
        let mut ui = None;

        let mut textarea = TextArea::default();
        let mut outputtextarea = TextArea::default();
        outputtextarea.set_cursor_style(Style::default());
        outputtextarea.set_line_number_style(Style::default().fg(Color::Yellow));

        let (stop_rx, stop_rc) = mpsc::channel();
        let (read_thread_tx, read_rx) = mpsc::channel();
        let (write_tx, write_thread_rx) = mpsc::channel();

        let _ = thread::spawn(|| term_io_loop(td, stop_rc, write_thread_rx, read_thread_tx));

        let res = 'event: loop {
            terminal.draw(|b| {
                if ui.is_none() {
                    ui = Some(UI::new(b));
                }
                ui.as_mut()
                    .unwrap()
                    .render(b, &mut textarea, &mut outputtextarea);
            })?;

            // Checke for any incoming bytes from the terminal device.
            if let Ok(res) = read_rx.try_recv() {
                for byte in &res {
                    if let Err(e) = self.parse_byte(*byte, &mut outputtextarea) {
                        break 'event Err(e);
                    };
                }
            }

            if let Ok(true) = event::poll(Duration::from_millis(1)) {
                let event = event::read()?;
                match event {
                    Event::Key(key) => {
                        if key.code == KeyCode::Esc {
                            return Ok(());
                        } else if key.code == KeyCode::Enter {
                            let mut line = textarea.lines()[0].clone();
                            textarea = TextArea::default();
                            line.push('\n');
                            write_tx.send(line.bytes().collect())?;
                            self.history.push(line);
                        } else if key.code == KeyCode::Char('d')
                            && key.modifiers == KeyModifiers::CONTROL
                        {
                            outputtextarea.move_cursor(tui_textarea::CursorMove::Bottom);
                            outputtextarea.move_cursor(tui_textarea::CursorMove::End);
                        } else {
                            textarea.input(key);
                        }
                    }
                    Event::Mouse(mouse_event) => match mouse_event.kind {
                        event::MouseEventKind::ScrollDown => {
                            outputtextarea.scroll((1, 0));
                        }
                        event::MouseEventKind::ScrollUp => {
                            outputtextarea.scroll((-1, 0));
                        }
                        _ => {}
                    },
                    Event::Resize(_, _) => {}
                    _ => {}
                }
            }
        };

        let _ = stop_rx.send(());

        res.map_err(|e| anyhow::anyhow!(e))
    }

    /// Parses a byte from the terminal device.
    pub fn parse_byte<'a>(
        &mut self,
        byte: u8,
        outputtextarea: &mut TextArea<'a>,
    ) -> std::io::Result<()> {
        let cursor_pos = outputtextarea.cursor();
        outputtextarea.move_cursor(tui_textarea::CursorMove::Bottom);
        outputtextarea.move_cursor(tui_textarea::CursorMove::End);
        let jumped = cursor_pos != outputtextarea.cursor();
        if byte == 10 {
            // new line
            if let Some(outfile) = &mut self.outfile {
                outfile.write_all(&mut format!("\n").into_bytes())?;
                outfile.flush()?;
            }
            outputtextarea.insert_newline();
        } else {
            let str = if let Ok(ch) = std::str::from_utf8(&[byte]) {
                format!("{}", ch.chars().next().unwrap())
            } else {
                // If it's not a vaild char, display out its hex value.
                format!("0x{byte:X}")
            };
            outputtextarea.insert_str(&str);
            if let Some(outfile) = &mut self.outfile {
                outfile.write_all(&mut str.into_bytes())?;
                outfile.flush()?;
            }
        }
        if jumped {
            outputtextarea.move_cursor(tui_textarea::CursorMove::Jump(
                cursor_pos.0 as u16,
                cursor_pos.1 as u16,
            ));
        }
        Ok(())
    }
}

impl UI {
    fn new<B: Backend>(f: &mut Frame<B>) -> Self {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(4),
                Constraint::Length(1),
            ])
            .split(f.size());

        UI {
            ouput_chunk: chunks[1],
            input_chunk: chunks[0],
        }
    }

    /// Renders all the widgets and their content.
    fn render<B: Backend>(
        &mut self,
        f: &mut Frame<B>,
        textarea: &mut TextArea,
        outputtextarea: &mut TextArea,
    ) {
        let input_block = Block::default().borders(Borders::ALL);
        let output_block = Block::default().borders(Borders::ALL);

        textarea.set_block(input_block);
        f.render_widget(textarea.widget(), self.input_chunk);

        outputtextarea.set_block(output_block);
        f.render_widget(outputtextarea.widget(), self.ouput_chunk);
    }
}
