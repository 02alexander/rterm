use tui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, StatefulWidget, Widget},
};

#[derive(Clone, Copy, Debug)]
pub enum Position {
    At(i32, i32), // At(line index, offset from bottom of line)
    Follow,
}

#[derive(Clone, Copy, Debug)]
pub enum Movement {
    ScrollUp,
    ScrollDown,
    Follow,
}

pub struct WrapTextState {
    pub position: Position,
    pub movement_queue: Vec<Movement>,
}

pub struct WrapText<'b> {
    pub lines: Vec<String>,
    pub block: Option<Block<'b>>,
}

pub struct WrappableTextWidget<'a, 'b> {
    pub lines: &'a Vec<String>,
    pub block: Option<Block<'b>>,
}

impl<'b> WrapText<'b> {
    pub fn widget(&mut self) -> WrappableTextWidget {
        WrappableTextWidget {
            lines: &self.lines,
            block: self.block.take(),
        }
    }
    pub fn set_block(&mut self, block: Block<'b>) {
        self.block = Some(block);
    }
}

impl WrapTextState {
    pub fn scroll_up(&mut self) {
        self.movement_queue.push(Movement::ScrollUp);
    }
    pub fn scroll_down(&mut self) {
        self.movement_queue.push(Movement::ScrollDown);
    }
    pub fn follow(&mut self) {
        self.movement_queue.push(Movement::Follow);
    }
}

impl Position {
    pub fn do_movement(
        &mut self,
        mov: Movement,
        line_number_width: usize,
        text_area: Rect,
        lines: &[String],
    ) {
        *self = match mov {
            Movement::ScrollUp => match self {
                Position::At(ref mut line, ref mut offset) => {
                    if *offset == 0 {
                        if *line != 0 {
                            *line -= 1;
                            let height = (lines[*line as usize].len() + line_number_width - 1)
                                / text_area.width as usize
                                + 1;
                            *offset = height as i32 - 1;
                        }
                    } else {
                        *offset -= 1
                    }
                    *self
                }
                Position::Follow => {
                    let (l, of) =
                        Position::follow_get_start_pos(text_area, lines, line_number_width);
                    Position::At(l, of)
                }
            },
            Movement::ScrollDown => match self {
                Position::At(ref mut line, ref mut offset) => {
                    let height = (lines[*line as usize].len() + line_number_width - 1)
                        / text_area.width as usize
                        + 1;
                    if *offset + 1 >= height as i32 {
                        if *line >= lines.len() as i32 - 1 {
                            *offset = (text_area.height as i32 - 1).min(*offset + 1);
                        } else {
                            *line += 1;
                            *offset = 0;
                        }
                    } else {
                        *offset += 1;
                    }
                    *self
                }
                Position::Follow => {
                    let (l, of) =
                        Position::follow_get_start_pos(text_area, lines, line_number_width);
                    Position::At(l, of)
                }
            },
            Movement::Follow => Position::Follow,
        }
    }

    /// Computes the start position given that we follow.
    pub fn follow_get_start_pos(
        text_area: Rect,
        lines: &[String],
        line_number_width: usize,
    ) -> (i32, i32) {
        let mut line_idx = -1;
        let mut offset = 0;
        let mut tot_height = 0;
        for line in lines.iter().rev() {
            let height =
                (line.len() as i32 + line_number_width as i32 - 1) / text_area.width as i32 + 1;
            tot_height += height as u16;
            line_idx += 1;
            if tot_height > text_area.height {
                offset = (tot_height - text_area.height) as i32;
                // if (tot_height - text_area.height) > 1 {
                //     line_idx += 1
                // }
                break;
            }
            if tot_height == text_area.height {
                break;
            }
        }
        (lines.len() as i32 - 1 - line_idx, offset)
    }
}

impl<'a, 'b> StatefulWidget for WrappableTextWidget<'a, 'b> {
    type State = WrapTextState;

    fn render(
        mut self,
        area: tui::layout::Rect,
        buf: &mut tui::buffer::Buffer,
        state: &mut Self::State,
    ) {
        let line_number_width = 4;

        let text_area = match self.block.take() {
            Some(b) => {
                let inner_area = b.inner(area);
                b.render(area, buf);
                inner_area
            }
            None => area,
        };

        for movement in &state.movement_queue {
            state
                .position
                .do_movement(*movement, line_number_width, text_area, self.lines);
        }
        state.movement_queue.clear();

        let (start_line_idx, offset) = match state.position {
            Position::At(line_idx, offset) => (line_idx, offset),
            Position::Follow => {
                Position::follow_get_start_pos(text_area, self.lines, line_number_width)
            }
        };
        let mut cur_row: i32 = -offset;
        for (line_idx_rel, line) in self.lines[start_line_idx as usize..].iter().enumerate() {
            let mut cur_col = 0;
            let mut tmp_string = String::new();
            if cur_row >= 0
                && cur_row < text_area.height as i32
                && text_area.width >= line_number_width as u16
            {
                buf.set_style(
                    Rect::new(
                        text_area.x,
                        (text_area.y as i32 + cur_row) as u16,
                        line_number_width as u16,
                        1,
                    ),
                    Style::default().fg(Color::Yellow),
                );
            }
            for (i, ch) in format!(" {:0>2} ", (start_line_idx as usize + line_idx_rel) % 100)
                .chars()
                .chain(line.chars())
                .enumerate()
            {
                if text_area.bottom() as i32 <= text_area.y as i32 + cur_row {
                    break;
                }

                if cur_row >= 0 {
                    tmp_string.push(ch);
                    buf.get_mut(text_area.x + cur_col, text_area.y + cur_row as u16)
                        .set_symbol(&tmp_string);
                    tmp_string.clear();
                }

                let is_last = i == line_number_width + line.len() - 1;
                cur_col += 1;
                if cur_col >= text_area.width && !is_last {
                    cur_col = 0;
                    cur_row += 1;
                }
            }
            cur_row += 1;
        }
    }
}

// aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
