mod app;
mod error;
mod net;
mod ui;

pub use app::App;
pub use crossterm::event::Event as InputEvent;

use argh::FromArgs;

use self::net::NetEvent;

/// play Skribbl.io-like games in the Termibbl
#[derive(FromArgs)]
#[argh(subcommand, name = "client")]
pub struct CliOpts {
    #[argh(positional)]
    ///username to connect as.
    pub username: Option<String>,

    #[argh(option, short = 'h')]
    /// address of server to connect to.
    pub host: Option<String>,
}

pub enum Event {
    Redraw,
    Input(InputEvent),
    Net(NetEvent),
}
