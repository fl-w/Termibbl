mod app;
<<<<<<< Updated upstream
mod app_server;
mod error;
=======
mod error;
mod net;
>>>>>>> Stashed changes
mod ui;

pub use app::App;
pub use crossterm::event::Event as InputEvent;

use argh::FromArgs;
<<<<<<< Updated upstream

/// play Skribbl.io-like games in the Termibbl
#[derive(FromArgs, Default)]
=======

use self::net::NetEvent;

/// play Skribbl.io-like games in the Termibbl
#[derive(FromArgs)]
>>>>>>> Stashed changes
#[argh(subcommand, name = "client")]
pub struct CliOpts {
    #[argh(positional)]
    ///username to connect as.
    pub username: Option<String>,

    #[argh(option, short = 'h')]
    /// address of server to connect to.
    pub host: Option<String>,
<<<<<<< Updated upstream

    #[argh(option, short = 'p')]
    /// port of the local server to connect
    pub port: Option<usize>,
=======
}

pub enum Event {
    Redraw,
    Input(InputEvent),
    Net(NetEvent),
>>>>>>> Stashed changes
}
