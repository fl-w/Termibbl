use std::net::{SocketAddr, ToSocketAddrs};

use crossterm::event::{KeyCode, KeyEvent};
use tui::{
    buffer::Buffer,
    layout::{Alignment, Constraint::*, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Paragraph, StatefulWidget, Widget},
    Frame,
};

use crate::{
    client::{app_server::ConnectionStatus, ui::widgets::input::InputWidget, App},
    data::Coord,
    do_nothing,
    message::{RoomRequest, ToServer},
};

use super::{
    centered_area,
    widgets::input::{Cursor, InputField},
    Action, Backend, View,
};

struct QueueLoader(usize);

impl Widget for QueueLoader {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(format!("Searching for a game{}", ".".repeat(self.0))).render(area, buf);
    }
}

pub struct StartMenu {
    pub host_input: InputField,
    pub username_input: InputField,
    pub error_msg: Option<String>,
    in_queue_loader: Option<QueueLoader>,
}

impl StartMenu {
    pub fn new() -> Self {
        let mut new = StartMenu {
            host_input: Default::default(),
            username_input: Default::default(),
            in_queue_loader: None,
            error_msg: Default::default(),
        };

        new.host_input.focus(true);
        new
    }

    pub fn on_connection_status_changed(&mut self, has_connection: bool) {
        self.host_input.focus(!has_connection);
        self.username_input.focus(has_connection);
    }

    pub fn tick(&mut self) {
        if let Some(loader) = &mut self.in_queue_loader {
            loader.0 += 1;
            loader.0 %= 3;
        }
    }

    pub fn join_queue(&mut self) {
        self.in_queue_loader = Some(QueueLoader(0));
        self.host_input.focus(false);
        self.username_input.focus(false);
    }

    pub fn leave_queue(&mut self) {
        self.in_queue_loader = None;
        self.username_input.focus(true);
    }

    pub fn in_queue(&self) -> bool { self.in_queue_loader.is_some() }
}

impl View for StartMenu {
    fn on_resize(&mut self, _: Coord) {}

    fn on_key_event(&mut self, event: KeyEvent) -> Action {
        let code = event.code;

        if self.host_input.has_focus() {
            self.host_input.on_key_event(code);

            if let KeyCode::Enter = code {
                if let Ok(Some(addr)) = self
                    .host_input
                    .content()
                    .to_socket_addrs()
                    .map(|mut addrs| addrs.next())
                {
                    return Box::new(move |app| app.connect_to_server(addr));
                }
            }

            do_nothing!()
        } else if self.username_input.has_focus() {
            self.username_input.on_key_event(code);
            if let KeyCode::Enter = code {
                let username = if self.username_input.is_empty() {
                    None
                } else {
                    Some(self.username_input.content().to_owned())
                };

                Box::new(move |app| {
                    app.server()
                        .send_message(ToServer::RequestRoom(username, RoomRequest::Find))
                })
            } else {
                do_nothing!()
            }
        } else {
            do_nothing!()
        }
    }

    fn on_mouse_event(&mut self, _event: crossterm::event::MouseEvent) -> Action { do_nothing!() }

    fn draw(&self, f: &mut Frame<Backend>, app: &App) {
        let area = f.size();
        let title_dimension = TitleWidget::dimension();

        let view_width = title_dimension.0;
        let view_height = title_dimension.1 + StartMenuInputWidget::HEIGHT + 6;

        let area = centered_area((view_width, view_height), area);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Length(title_dimension.1),
                    Length(3),
                    Length(StartMenuInputWidget::HEIGHT),
                    Length(3),
                    Length(StartMenuHelpWidget::HEIGHT),
                ]
                .as_ref(),
            )
            .split(area);

        let mut cursor = Cursor::default();

        f.render_widget(TitleWidget::default(), layout[0]);

        f.render_stateful_widget(
            StartMenuInputWidget::new(&self, &app.server().connection_status()),
            layout[2],
            &mut cursor,
        );

        f.render_widget(StartMenuHelpWidget::new(&self), layout[4]);

        if let Some((x, y)) = cursor.take() {
            f.set_cursor(x, y);
        }
    }
}

#[derive(Default)]
pub struct TitleWidget;

impl TitleWidget {
    const TITLE: [&'static str; 6] = [
        r"▄▄▄▄▄▄▄                        ▀    █      █      ▀▀█   ",
        r"   █     ▄▄▄    ▄ ▄▄  ▄▄▄▄▄  ▄▄▄    █▄▄▄   █▄▄▄     █   ",
        r"   █    █▀  █   █▀  ▀ █ █ █    █    █▀ ▀█  █▀ ▀█    █   ",
        r"   █    █▀▀▀▀   █     █ █ █    █    █   █  █   █    █   ",
        r"   █    ▀█▄▄▀   █     █ █ █  ▄▄█▄▄  ██▄█▀  ██▄█▀    ▀▄▄ ",
        r"                                                        ",
    ];
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const X_OFFSET: u16 = 44;

    pub fn dimension() -> (u16, u16) { (Self::TITLE[0].len() as u16, Self::TITLE.len() as u16 + 1) }
}

impl Widget for TitleWidget {
    fn render(self, mut area: tui::layout::Rect, buf: &mut Buffer) {
        Paragraph::new(
            Self::TITLE
                .iter()
                .cloned()
                .map(Spans::from)
                .collect::<Vec<Spans>>(),
        )
        .alignment(Alignment::Center)
        .render(area, buf);

        if area.width >= Self::X_OFFSET {
            area.x += Self::X_OFFSET;
            area.y += Self::TITLE.len() as u16;
            area.width -= Self::X_OFFSET;

            Paragraph::new(Span::styled(
                format!("version: {}", Self::VERSION),
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::BOLD),
            ))
            .alignment(Alignment::Left)
            .render(area, buf);
        }
    }
}

pub struct StartMenuInputWidget<'a> {
    status: &'a ConnectionStatus,
    start_menu: &'a StartMenu,
}

impl<'a> StartMenuInputWidget<'a> {
    const HEIGHT: u16 = 2;

    fn new(start_menu: &'a StartMenu, status: &'a ConnectionStatus) -> Self {
        Self { status, start_menu }
    }
}

impl StatefulWidget for StartMenuInputWidget<'_> {
    type State = Cursor;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let start_menu = self.start_menu;
        let connection_status = match self.status {
            ConnectionStatus::NotConnected => ("Not connected", Color::DarkGray),
            ConnectionStatus::Connecting => ("Connecting..", Color::Gray),
            ConnectionStatus::NotFound => ("Not Found", Color::Red),
            ConnectionStatus::Dropped => ("Dropped", Color::Red),
            ConnectionStatus::Timedout => ("Timed Out", Color::Yellow),
            ConnectionStatus::Connected => ("Connected", Color::LightGreen),
        };

        let widgets = vec![
            InputWidget::new(
                "Server addr:  ",
                match start_menu.host_input.content().to_socket_addrs() {
                    Err(_) => ("Use 'ip:port' syntax", Color::Yellow),
                    Ok(_) => connection_status,
                },
                &start_menu.host_input,
            ),
            InputWidget::new(
                "Player name:  ",
                if start_menu.host_input.content().is_empty() {
                    ("Name cannot be empty", Color::DarkGray)
                } else {
                    ("Ready?!", Color::Gray)
                },
                &start_menu.username_input,
            ),
        ];

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints((0..widgets.len()).map(|_| Length(1)).collect::<Vec<_>>())
            .horizontal_margin(5)
            .split(area);

        for (i, widget) in widgets.into_iter().enumerate() {
            widget.render(layout[i], buf, state);
        }
    }
}

pub struct StartMenuHelpWidget<'a> {
    start_menu: &'a StartMenu,
}

impl<'a> StartMenuHelpWidget<'a> {
    const HEIGHT: u16 = 0;
    fn new(start_menu: &'a StartMenu) -> Self { Self { start_menu } }
}

impl<'a> Widget for StartMenuHelpWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {}
}
