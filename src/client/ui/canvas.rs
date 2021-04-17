use std::collections::HashMap;

use tui::{buffer::Buffer, layout::Rect};

use crate::world::{Color, Coord, Draw};

pub const PALETTE: [Color; 16] = [
    Color::White,
    Color::Gray,
    Color::DarkGray,
    Color::Black,
    Color::Red,
    Color::LightRed,
    Color::Green,
    Color::LightGreen,
    Color::Blue,
    Color::LightBlue,
    Color::Yellow,
    Color::LightYellow,
    Color::Cyan,
    Color::LightCyan,
    Color::Magenta,
    Color::LightMagenta,
];

#[derive(Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum PaintTool {
    Pen,
    Fill,
    Eraser,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Palette {
    pub paint_tool: PaintTool,
    pub selected_color_index: usize,
    pub palette: [Color; 16],
}

impl Palette {
    pub fn new(palette: [Color; 16]) -> Self {
        Self {
            paint_tool: PaintTool::Pen,
            selected_color_index: 0,
            palette: PALETTE,
        }
    }

    pub fn selected_color(&self) -> Color { self.palette[self.selected_color_index] }
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct TermCanvas {
    pub width: u16,
    pub height: u16,
    view_offset_x: u16,
    view_offset_y: u16,
    buffer: HashMap<Coord, Color>,
    background_color: Option<Color>,
    should_show_grid: bool,
    is_ascii_mode: bool,
    last_mouse_pos: Option<(isize, isize)>,
}

impl TermCanvas {
    pub fn new(width: u16, height: u16) -> Self {
        TermCanvas {
            width,
            height,
            view_offset_x: 0,
            view_offset_y: 0,
            buffer: HashMap::new(),
            background_color: None,
            should_show_grid: false,
            is_ascii_mode: false,
            last_mouse_pos: None,
        }
    }

    pub fn dimensions(&self) -> Coord { (self.width, self.height) }

    pub fn clear(&mut self) { self.buffer.clear() }

    pub fn showing_grid(&self) -> bool { self.should_show_grid }

    pub fn within_bounds(&self, (x, y): &Coord) -> bool { x < &self.width && y < &self.height }

    pub fn toggle_grid(&mut self) { self.should_show_grid = !self.should_show_grid; }

    pub fn resize_canvas(&mut self, size: Coord) {
        self.width = size.0;
        self.height = size.1;
        self.view_offset_x = 0;
        self.view_offset_y = 0;
    }

    pub fn bg(mut self, bg: Color) -> Self {
        self.background_color = Some(bg);

        self
    }

    pub fn draw(&mut self, draw_action: Draw) {
        match draw_action {
            Draw::Clear => self.clear(),
            Draw::Erase(point) => self.erase(point),
            Draw::Paint { ref points, color } => self.paint(points, color),
        }
    }

    pub fn erase(&mut self, point: Coord) { self.buffer.remove(&point); }

    pub fn paint(&mut self, points: &[Coord], color: Color) {
        for point in points.iter() {
            if self.within_bounds(point) {
                self.buffer.insert(*point, color);
            }
        }
    }

    pub fn resize(&mut self, size: Rect) {}

    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        if let Some(color) = &self.background_color {
            // buf.set_style(area, Style::default().bg((*color).into()))
        }

        // draw grid
        if self.showing_grid() {
            // for (x, y) in (0..self.canvas.height - 1)
            //     .zip(0..self.canvas.width - 1)
            //     .step_by(2)
            // {
            //     buf.get_mut(x, y).set_bg(Color::DarkGray);
            // }
        }

        for x in 0..area.width {
            for y in 0..area.height {
                let (global_x, global_y) = (area.x + x, area.y + y);

                let offset_x = self.view_offset_x + x;
                let offset_y = self.view_offset_y + y;

                if self.within_bounds(&(offset_x, offset_y)) {
                    // if this point is drawn on or canvas
                    if let Some(color) = self.buffer.get(&(offset_x, offset_y)) {
                        buf.get_mut(offset_x, offset_y).set_bg(color.clone().into());
                    }
                } else {
                    buf.get_mut(global_x, global_y)
                        .set_fg(tui::style::Color::Red)
                        .set_char('∅');
                }
            }
        }
    }
}
