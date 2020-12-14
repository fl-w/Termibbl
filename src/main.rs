pub mod client;
pub mod data;
pub mod message;
pub mod server;

use actix::prelude::*;
use argh::FromArgs;
use std::io::{stdout, Write};
use std::path::PathBuf;

use crossterm::{
    event::{read, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, MouseEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    Result,
};

use tui::{backend::CrosstermBackend, Terminal};

use client::app::ServerSession;
use data::Username;
pub use serde::{Deserialize, Serialize};

#[derive(FromArgs)]
/// A Skribbl.io-alike for the terminal
struct Opt {
    #[argh(subcommand)]
    cmd: SubOpt,
}

#[derive(FromArgs)]
#[argh(subcommand)]
enum SubOpt {
    Server(server::CliOpts),
    Client(client::CliOpts),
}

#[actix_rt::main]
async fn main() {
    let cli: Opt = argh::from_env();

    match cli.cmd {
        SubOpt::Client(opts) => client::run_with_opts(opts),
        SubOpt::Server(opts) => server::run_with_opts(opts).await.unwrap(),
    }
}

pub enum ClientEvent {
    MouseInput(MouseEvent),
    KeyInput(KeyEvent),
    ServerMessage(message::ToClientMsg),
}

async fn run_client(addr: &str, username: Username) -> client::error::Result<()> {
    let (mut client_evt_send, client_evt_recv) = tokio::sync::mpsc::channel::<ClientEvent>(1);

    let mut app =
        ServerSession::establish_connection(addr, username, client_evt_send.clone()).await?;

    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    execute!(stdout(), EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    tokio::spawn(async move {
        app.run(&mut terminal, client_evt_recv).await.unwrap();
    });
    loop {
        match read()? {
            Event::Key(evt) => match evt {
                KeyEvent {
                    code: KeyCode::Esc,
                    modifiers: _,
                } => break,
                _ => {
                    let _ = client_evt_send.send(ClientEvent::KeyInput(evt)).await;
                }
            },
            Event::Mouse(evt) => {
                let _ = client_evt_send.send(ClientEvent::MouseInput(evt)).await;
            }
            _ => {}
        }
    }

    execute!(stdout(), DisableMouseCapture)?;
    execute!(stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}
