use std::{
    fs::File,
    io::{self, Read, Write},
    sync::mpsc::{self, Receiver, Sender},
    thread::{self},
    time::Duration,
};

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ordered_float::OrderedFloat;
use regex::Regex;
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::Span,
    widgets::{Axis, Block, Borders, Chart, Dataset, GraphType},
    Frame, Terminal,
};
use tui_textarea::TextArea;

use crate::{
    termdev::TerminalDevice,
    wraptext::{Position, WrapText, WrapTextState},
};

pub struct App {
    outfile: Option<File>,
    history: Vec<String>,
    cur_line: String,
    pub grapher: Option<Grapher>,
}

pub struct Grapher {
    pub data: Vec<(f64, f64)>,
    pub value_pattern: Regex,
    pub window_len: usize,
    pub window: [f64; 2],
}

pub struct UI {
    input_chunk: Rect,
    ouput_chunk: Rect,
    graph_chunk: Option<Rect>,
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
                Err(e) => {
                    if e.kind() != io::ErrorKind::WouldBlock {
                        { Err(e) }?;
                    }
                }
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
            break;
        }
    }
    let _ = read_thread_stop_tx.send(());
    let _ = write_thread_stop_tx.send(());
    term_reader_handle.join().unwrap()?;
    term_writer_handle.join().unwrap()?;
    Ok(())
}

impl App {
    pub fn new(outfile: Option<File>) -> Self {
        App {
            outfile,
            cur_line: String::new(),
            history: Vec::new(),
            grapher: None,
        }
    }

    pub fn run<B: Backend>(
        &mut self,
        td: TerminalDevice,
        terminal: &mut Terminal<B>,
    ) -> anyhow::Result<()> {
        let mut ui = None;

        let mut textarea = TextArea::default();
        let mut wraptext = WrapText {
            lines: vec![String::new()],
            block: None,
        };
        let mut text_state = WrapTextState {
            position: Position::Follow,
            movement_queue: Vec::new(),
        };

        // let mut outputtextarea = TextArea::default();
        // outputtextarea.set_cursor_style(Style::default());
        // outputtextarea.set_line_number_style(Style::default().fg(Color::Yellow));

        let (stop_rx, stop_rc) = mpsc::channel();
        let (read_thread_tx, read_rx) = mpsc::channel();
        let (write_tx, write_thread_rx) = mpsc::channel();
        let mut update = true;

        let _ = thread::spawn(|| term_io_loop(td, stop_rc, write_thread_rx, read_thread_tx));
        let res = 'event: loop {
            if update {
                update = false;
                terminal.draw(|b| {
                    if ui.is_none() {
                        ui = Some(UI::new(b, self.grapher.is_some()));
                    }
                    ui.as_mut().unwrap().render(
                        b,
                        &mut textarea,
                        &mut wraptext,
                        &mut text_state,
                        &mut self.grapher,
                    )
                })?;
            }

            // Checke for any incoming bytes from the terminal device.
            if let Ok(res) = read_rx.try_recv() {
                update = true;
                for byte in &res {
                    if let Err(e) = self.parse_byte(*byte, &mut wraptext) {
                        break 'event Err(e);
                    };
                }
            }

            if let Ok(true) = event::poll(Duration::from_millis(1)) {
                let event = event::read()?;
                let mut should_update = true;
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
                            text_state.follow();
                            // outputtextarea.move_cursor(tui_textarea::CursorMove::Bottom);
                            // outputtextarea.move_cursor(tui_textarea::CursorMove::End);
                        } else {
                            textarea.input(key);
                        }
                    }
                    Event::Mouse(mouse_event) => match mouse_event.kind {
                        event::MouseEventKind::ScrollDown => {
                            text_state.scroll_down();
                        }
                        event::MouseEventKind::ScrollUp => {
                            text_state.scroll_up();
                        }
                        _ => should_update = false,
                    },
                    Event::Resize(w, h) => {
                        if let Some(ui) = ui.as_mut() {
                            ui.update_size(w, h, self.grapher.is_some());
                        }
                    }
                    _ => should_update = false,
                }
                if should_update {
                    update = true;
                }
            }
        };

        let _ = stop_rx.send(());

        res.map_err(|e| anyhow::anyhow!(e))
    }

    /// Parses a byte from the terminal device.
    pub fn parse_byte(&mut self, byte: u8, wraptext: &mut WrapText) -> std::io::Result<()> {
        // let cursor_pos = wraptext.cursor();
        // wraptext.move_cursor(tui_textarea::CursorMove::Bottom);
        // wraptext.move_cursor(tui_textarea::CursorMove::End);
        // let jumped = cursor_pos != wraptext.cursor();
        if byte == 10 {
            // new line
            if let Some(outfile) = &mut self.outfile {
                outfile.write_all(&"\n".to_string().into_bytes())?;
                outfile.flush()?;
            }
            // wraptext.insert_newline();
            wraptext.lines.push(String::new());
            if let Some(grapher) = &mut self.grapher {
                if let Some(captures) = grapher.value_pattern.captures(&self.cur_line) {
                    if let Some(capture) = captures.get(0) {
                        if let Ok(val) = capture.as_str().parse() {
                            if grapher.data.len() as f64 + grapher.window_len as f64 / 10.0
                                > grapher.window[1]
                            {
                                grapher.window[0] += 1.0;
                                grapher.window[1] += 1.0;
                            }
                            grapher.data.push((grapher.data.len() as f64, val));
                        }
                    }
                }
            }
            self.cur_line.clear();
        } else {
            let str = if let Ok(ch) = std::str::from_utf8(&[byte]) {
                format!("{}", ch.chars().next().unwrap())
            } else {
                // If it's not a vaild char, display out its hex value.
                format!("0x{byte:X}")
            };
            wraptext.lines.last_mut().unwrap().push_str(&str);
            self.cur_line.push_str(&str);
            if let Some(outfile) = &mut self.outfile {
                outfile.write_all(&str.into_bytes())?;
                outfile.flush()?;
            }
        }
        // if jumped {
        //     wraptext.move_cursor(tui_textarea::CursorMove::Jump(
        //         cursor_pos.0 as u16,
        //         cursor_pos.1 as u16,
        //     ));
        // }
        Ok(())
    }
}

impl UI {
    fn new(f: &mut Frame<impl Backend>, graph: bool) -> Self {
        let chunks = UI::generate_chunks(f.size(), graph);
        let graph_chunk = if graph { Some(chunks[2]) } else { None };
        UI {
            ouput_chunk: chunks[1],
            input_chunk: chunks[0],
            graph_chunk,
        }
    }

    pub fn generate_chunks(rect: Rect, graph: bool) -> Vec<Rect> {
        let mut constraints = vec![Constraint::Length(3)];
        if graph {
            constraints.push(Constraint::Percentage(50));
            constraints.push(Constraint::Percentage(50));
        } else {
            constraints.push(Constraint::Min(4));
        }
        Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(rect)
    }

    fn update_size(&mut self, width: u16, height: u16, graph: bool) {
        let chunks = UI::generate_chunks(Rect::new(0, 0, width, height), graph);
        let graph_chunk = if graph { Some(chunks[2]) } else { None };
        *self = UI {
            ouput_chunk: chunks[1],
            input_chunk: chunks[0],
            graph_chunk,
        }
    }

    /// Renders all the widgets and their content.
    fn render<B: Backend>(
        &mut self,
        f: &mut Frame<B>,
        textarea: &mut TextArea,
        wraptext: &mut WrapText,
        text_state: &mut WrapTextState,
        grapher: &mut Option<Grapher>,
    ) {
        let input_block = Block::default().borders(Borders::ALL);
        let output_block = Block::default().borders(Borders::ALL);

        textarea.set_block(input_block);
        f.render_widget(textarea.widget(), self.input_chunk);

        wraptext.set_block(output_block);
        f.render_stateful_widget(wraptext.widget(), self.ouput_chunk, text_state);

        if let Some(graph_chunk) = self.graph_chunk {
            let graph_block = Block::default().borders(Borders::ALL);
            let grapher = grapher.as_ref().unwrap();
            let visible_data = &grapher.data
                [0.max(grapher.data.len() as i64 - grapher.window_len as i64) as usize..];
            let datasets = vec![Dataset::default()
                .marker(symbols::Marker::Braille)
                .style(Style::default().fg(Color::Yellow))
                .graph_type(GraphType::Line)
                .data(visible_data)];

            let min = visible_data
                .iter()
                .min_by_key(|(_x, y)| OrderedFloat(*y))
                .map(|x| x.1)
                .unwrap_or(-1.0);
            let max = visible_data
                .iter()
                .max_by_key(|(_x, y)| OrderedFloat(*y))
                .map(|x| x.1)
                .unwrap_or(1.0);
            let size = max - min;
            let min = min - 0.1 * size - 0.001 * max.abs().max(min.abs());
            let max = max + 0.1 * size + 0.001 * max.abs().max(min.abs());
            let mean = (max + min) / 2.0;

            let chart = Chart::new(datasets)
                .block(graph_block)
                .x_axis(Axis::default().bounds(grapher.window).title("X axis"))
                .y_axis(Axis::default().bounds([min, max]).labels(vec![
                    Span::styled(
                        format!("{min:.4}"),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(format!("{mean:.4}")),
                    Span::styled(
                        format!("{max:.4}"),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ]));
            f.render_widget(chart, graph_chunk);
        }
    }
}
