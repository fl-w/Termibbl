use std::collections::HashMap;

use tui::{buffer::Buffer, layout::Rect, style::Style};

use crate::{
    client::ui::elements::Element,
    data::{Color, Coord},
    message::Draw,
};

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

#[derive(Copy, Clone, Debug)]
pub enum PaintTool {
    Pen,
    Fill,
    Eraser,
}

#[derive(Clone, Debug)]
pub struct Palette {
    pub paint_tool: PaintTool,
    pub colors: [Color; 16],
    selected_color_index: usize,
    bounds: Rect,
    hidden: bool,
}

impl Palette {
    pub const HEIGHT: u16 = 2;
    pub fn new(colors: [Color; 16]) -> Self {
        Self {
            paint_tool: PaintTool::Pen,
            colors,
            selected_color_index: 0,
            bounds: Rect::default(),
            hidden: false,
        }
    }
    pub fn toggle_hide(&mut self) { self.hidden = !self.hidden }

    pub fn is_hidden(&self) -> bool { self.hidden }

    pub fn selected_color(&self) -> Color { self.colors[self.selected_color_index] }

    pub fn set_selected_color_index(&mut self, i: usize) {}

    pub fn select_color_on_coord(&mut self, coord_clicked: Coord) {
        let swatch_size = self.bounds().width / self.colors.len() as u16;

        let selected_color_index = (coord_clicked.0 / swatch_size) as usize;

        if selected_color_index < self.colors.len() {
            self.selected_color_index = selected_color_index;
        }
    }
}

impl Element for Palette {
    fn resize(&mut self, bounds: Rect) { self.bounds = bounds; }
    fn bounds(&self) -> Rect { self.bounds }
    fn render(&self, buf: &mut Buffer) {
        // draw palette
        let swatch_size = self.bounds().width / self.colors.len() as u16;

        for (idx, col) in self.colors.iter().enumerate() {
            for offset in 0..swatch_size {
                for y in 0..Self::HEIGHT - 1 {
                    buf.get_mut((swatch_size * idx as u16) + offset, y)
                        .set_bg((*col).into());
                }
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TermCanvas {
    pub width: u16,
    pub height: u16,
    rect: Rect,
    view_offset_x: u16,
    view_offset_y: u16,
    buffer: HashMap<Coord, Color>,
    background_color: Option<Color>,
    should_show_grid: bool,
    is_ascii_mode: bool,
    last_mouse_pos: Option<(isize, isize)>,
}

impl TermCanvas {
    pub fn new((width, height): (u16, u16)) -> Self {
        TermCanvas {
            width,
            height,
            view_offset_x: 0,
            view_offset_y: 0,
            rect: Rect::default(),
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

    pub fn reset_mouse(&mut self) { self.last_mouse_pos = None }

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

    pub fn apply(&mut self, draw_action: Draw) {
        match draw_action {
            Draw::Clear => self.clear(),
            Draw::Erase(point) => self.erase(point),
            Draw::Paint { ref points, color } => self.paint(points, color),
        }
    }

    pub fn draw(&mut self, pos: Coord, color: Color) -> Draw {
        let mouse_pos = (pos.0 as isize, pos.1 as isize);
        let old_mouse_pos = self.last_mouse_pos.replace(mouse_pos);

        let points = line_drawing::Bresenham::new(old_mouse_pos.unwrap_or(mouse_pos), mouse_pos)
            // .skip(if has_prev_point { 1 } else { 0 })
            .map(|(x, y)| (x as u16, y as u16))
            .collect::<Vec<_>>();

        // apply draw to canvas before sending to server.
        self.paint(points.as_slice(), color);

        Draw::Paint { color, points }
    }

    pub fn erase(&mut self, point: Coord) { self.buffer.remove(&point); }

    pub fn paint(&mut self, points: &[Coord], color: Color) {
        for point in points.iter() {
            if self.within_bounds(point) {
                self.buffer.insert(*point, color);
            }
        }
    }
}

impl Element for TermCanvas {
    fn resize(&mut self, bounds: Rect) { self.rect = bounds }

    fn render(&self, buf: &mut Buffer) {
        let area = self.bounds();

        // draw canvas background
        if let Some(color) = &self.background_color {
            buf.set_style(area, Style::default().bg((*color).into()))
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
                let (offset_x, offset_y) = (area.x + x, area.y + y);

                let global_x = self.view_offset_x + offset_x;
                let global_y = self.view_offset_y + offset_y;

                // if this point is drawn on or canvas
                if self.within_bounds(&(global_x, global_y)) {
                    if let Some(color) = self.buffer.get(&(global_x, global_y)) {
                        buf.get_mut(offset_x, offset_y).set_bg((*color).into());
                    }
                } else {
                    buf.get_mut(global_x, global_y)
                        .set_fg(tui::style::Color::Red)
                        .set_char('∅');
                }
            }
        }
    }

    fn bounds(&self) -> Rect { self.rect }
}
