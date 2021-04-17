use std::net::SocketAddr;

use crossterm::event::{KeyCode, KeyEvent};
use tui::{
    backend::Backend,
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Paragraph, StatefulWidget, Widget},
    Frame,
};

use crate::{
    client::{
        app::{AppServer, ConnectionStatus},
        App,
    },
    do_nothing,
    message::{RoomRequest, ToServer},
    world::Coord,
};

use super::{
    input::{centered_area, Cursor, InputText},
    Element, ElementHolder, View,
};

#[derive(Default, Debug)]
pub struct StartMenu {
    pub host_input: InputText,
    pub username_input: InputText,
}

impl StartMenu {
    pub fn new(host: Option<String>, username: Option<String>) -> Self {
        let mut new = StartMenu::default();

        if let Some(host) = host {
            new.host_input.set_content(host)
        }

        if let Some(username) = username {
            new.username_input.set_content(username)
        }

        new
    }
}

impl View for StartMenu {
    fn on_resize(&mut self, size: Coord) -> Box<dyn Fn(&mut App)> { do_nothing!() }

    fn on_key_event(&mut self, event: KeyEvent) -> Box<dyn Fn(&mut App)> {
        let code = event.code;

        if self.host_input.has_focus() {
            self.host_input.on_key_event(code);

            if let KeyCode::Enter = code {
                if let Ok(addr) = self.host_input.content().parse::<SocketAddr>() {
                    return Box::new(move |app| app.connect_to_server(addr));
                }
            }

            Box::new(|app| app.reset_connection_state())
        } else if let KeyCode::Enter = code {
            let username = self.username_input.content().to_owned();

            Box::new(move |app| {
                app.server_mut().send_message(ToServer::RequestRoom(
                    Some(username.clone()),
                    RoomRequest::Join("main".to_owned()),
                ))
            })
        } else {
            self.username_input.on_key_event(code);
            do_nothing!()
        }
    }

    fn on_mouse_event(&mut self, _event: crossterm::event::MouseEvent) -> Box<dyn Fn(&mut App)> {
        do_nothing!()
    }
}

impl ElementHolder for StartMenu {
    fn element_in<E: Element>(&self, coord: Coord) -> Option<&E> { todo!() }
    fn element_in_mut<E: Element>(&mut self, coord: Coord) -> Option<&mut E> { todo!() }
}

pub fn draw_start_view<B>(f: &mut Frame<B>, start_menu: &StartMenu, server: &AppServer)
where
    B: Backend,
{
    let area = f.size();
    let title_dimension = TitleWidget::dimension();

    let view_width = title_dimension.0;
    let view_height = title_dimension.1 + StartMenuInputWidget::HEIGHT + 6;

    let area = centered_area((view_width, view_height), area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(title_dimension.1),
                Constraint::Length(3),
                Constraint::Length(StartMenuInputWidget::HEIGHT),
                Constraint::Length(3),
                Constraint::Length(StartMenuHelpWidget::HEIGHT),
            ]
            .as_ref(),
        )
        .split(area);

    let mut cursor = Cursor::default();

    f.render_widget(TitleWidget::default(), layout[0]);
    f.render_stateful_widget(
        StartMenuInputWidget::new(server, start_menu),
        layout[2],
        &mut cursor,
    );

    f.render_widget(StartMenuHelpWidget::new(start_menu), layout[4]);

    if let Some((x, y)) = cursor.take() {
        f.set_cursor(x, y);
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
    start_menu: &'a StartMenu,
    server: &'a AppServer,
}

impl<'a> StartMenuInputWidget<'a> {
    const HEIGHT: u16 = 2;

    fn new(server: &'a AppServer, start_menu: &'a StartMenu) -> Self { Self { start_menu, server } }
}

impl StatefulWidget for StartMenuInputWidget<'_> {
    type State = Cursor;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let start_menu = self.start_menu;
        let server = self.server;

        let widgets = vec![
            InputWidget::new(
                ServerAddrInputWidget::LABEL,
                &start_menu.host_input,
                server,
                Box::new(ServerAddrInputWidget::hint),
            ),
            InputWidget::new(
                UsernameInputWidget::LABEL,
                &start_menu.username_input,
                server,
                Box::new(UsernameInputWidget::hint),
            ),
        ];

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                (0..widgets.len())
                    .map(|_| Constraint::Length(1))
                    .collect::<Vec<_>>(),
            )
            .horizontal_margin(5)
            .split(area);

        for (i, widget) in widgets.into_iter().enumerate() {
            widget.render(layout[i], buf, state);
        }
    }
}

pub type Hint = (&'static str, Color);

pub struct InputWidget<'a> {
    label: &'a str,
    input: &'a InputText,
    // callback: &'a fn() -> (&'static str, Color),
    hint_fn: Box<dyn Fn(&'a InputText, &'a AppServer) -> Hint>,
    server: &'a AppServer,
}

impl<'a> InputWidget<'a> {
    fn new(
        label: &'a str,
        input: &'a InputText,
        server: &'a AppServer,
        hint_fn: Box<dyn Fn(&'a InputText, &'a AppServer) -> Hint>,
    ) -> Self {
        Self {
            label,
            input,
            hint_fn,
            server,
        }
    }
    // start_menu: &'a StartMenu, server: &'a AppServer
}

impl StatefulWidget for InputWidget<'_> {
    type State = Cursor;

    fn render(self, area: Rect, buf: &mut Buffer, cursor: &mut Cursor) {
        let label = Spans::from(vec![
            Span::raw(self.label),
            Span::styled(
                self.input.content(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]);

        Paragraph::new(label)
            .alignment(Alignment::Left)
            .render(area, buf);

        let (message, hint_color) = (self.hint_fn)(self.input, self.server);
        let hint = Span::styled(message, Style::default().fg(hint_color));
        Paragraph::new(hint)
            .alignment(Alignment::Right)
            .render(area, buf);

        if self.input.has_focus() {
            cursor.set(
                area.x + (self.label.len() + self.input.cursor()) as u16,
                area.y,
            );
        }
    }
}

pub struct ServerAddrInputWidget;

impl ServerAddrInputWidget {
    const LABEL: &'static str = "Server addr:  ";

    fn hint(input: &InputText, server: &AppServer) -> Hint {
        if input.content().is_empty() {
            ("Not connected", Color::DarkGray)
        } else {
            match input.content().parse::<std::net::SocketAddr>() {
                Err(_) => ("Use 'ip:port' syntax", Color::Yellow),
                Ok(_) => match server.connection_status() {
                    ConnectionStatus::NotConnected => ("Not connected", Color::DarkGray),
                    ConnectionStatus::Connecting => ("Connecting..", Color::Gray),
                    ConnectionStatus::NotFound => ("Not Found", Color::Red),
                    ConnectionStatus::Dropped => ("Dropped", Color::Red),
                    ConnectionStatus::TimedOut => ("Timed Out", Color::Yellow),
                    ConnectionStatus::Connected => ("Connected", Color::LightGreen),
                },
            }
        }
    }
}

pub struct UsernameInputWidget;

impl UsernameInputWidget {
    const LABEL: &'static str = "Player name:  ";

    fn hint(input: &InputText, _: &AppServer) -> Hint {
        if input.content().is_empty() {
            ("Name cannot be empty", Color::DarkGray)
        } else {
            ("Ready?!", Color::Gray)
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
