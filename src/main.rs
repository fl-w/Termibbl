<<<<<<< Updated upstream
#![allow(dead_code, unused_variables)]
mod client;
mod data;
mod encoding;
mod events;
mod message;
mod server;
mod utils;

use argh::FromArgs;
use client::App;

use std::{error::Error, net::SocketAddr};
=======
// #![feature(associated_type_bounds)]
mod client;
mod events;
mod message;
mod server;
mod world;

use client::App;
use events::EventSender;
use server::GameServer;
use world::GameOpts;

use argh::FromArgs;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{error::Error, io};
>>>>>>> Stashed changes

/// A Skribbl.io-alike for the terminal
#[derive(FromArgs)]
struct Opt {
    #[argh(subcommand)]
    cmd: Option<SubOpt>,
}

#[derive(FromArgs)]
#[argh(subcommand)]
enum SubOpt {
    Client(client::CliOpts),
<<<<<<< Updated upstream
    Server(server::CliOpts),
=======
}

async fn process_input_events(app_event_tx: EventSender<client::Event>) {
    loop {
        // blocking read
        let event = crossterm::event::read().unwrap();

        app_event_tx.send(client::Event::Input(event));
    }
}

async fn process_ctrl_c(server_tx: &EventSender<server::Message>) {
    tokio::signal::ctrl_c().await;
    println!("✨ Ctrl-C received. Stopping..");
    server_tx.send_immediate(server::Message::CtrlC)
>>>>>>> Stashed changes
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
<<<<<<< Updated upstream
    let cli: Opt = argh::from_env();
    println!("hello world");

    // set default command to 'client'
    let cmd = cli
        .cmd
        .unwrap_or_else(|| SubOpt::Client(client::CliOpts::default()));

    match cmd {
        SubOpt::Client(opt) => {
            let mut app = App::default();

            let localhost = opt.port.map(|port| format!("127.0.0.1:{}", port));
            if let Some(addr) = opt.host.or(localhost) {
                app.set_host_input(addr.clone());

                if let Ok(addr) = addr.parse::<SocketAddr>() {
                    app.connect_to_server(addr);
                }
            }

            if let Some(name) = opt.username {
                app.set_name_input(name)
            }

            app.start().await?;
        }

        SubOpt::Server(cli) => {
            let port = cli.port;
            let default_game_opts = data::GameOpts {
                dimensions: cli.dimensions,
                number_of_rounds: cli.rounds,
                draw_time: cli.draw_time as usize,
                only_custom_words: false,
            };

            server::run(port, default_game_opts, cli.words.take()).await?;
        }
    }
=======
    pretty_env_logger::init();

    let cli: Opt = argh::from_env();

    match cli.cmd {
        SubOpt::Server(opt) => {
            let port = opt.port;

            // display public ip
            if opt.display_public_ip {
                tokio::spawn(async move {
                    if let Ok(res) = reqwest::get("http://ifconfig.me").await {
                        if let Ok(ip) = res.text().await {
                            println!("Your public IP is {}:{}", ip, port);
                            println!("You can find out your private IP by running \"ip addr\" in the terminal");
                        }
                    }
                });
            }

            let default_game_opts: GameOpts = opt.into();
            let server = GameServer::new(default_game_opts);
            let addr = format!("127.0.0.1:{}", port);

            // listen for ctrl_c
            tokio::spawn(process_ctrl_c(server.sender()));

            println!("🚀 Running Termibbl server on port {}...", port);
            server.listen_on(&addr).await?;
        }

        SubOpt::Client(opt) => {
            let mut app = App::from_args(opt);
            let mut stdout = io::stdout();

            enable_raw_mode()?;
            execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

            // handle term events
            tokio::spawn(process_input_events(app.sender().clone()));

            app.run().await?;

            execute!(stdout, LeaveAlternateScreen, DisableMouseCapture)?;
            disable_raw_mode()?;
        }
    };
>>>>>>> Stashed changes

    Ok(())
}

// fn words<'a>(words: &'a Vec<String>) -> impl std::iter::Iterator<Item = &'a str> {
//     let mut num = 0;
//     std::iter::from_fn(move || {
//         let result;
//         if num < n {
//             result = Some(num);
//             num += 1
//         } else {
//             result = None
//         }
//         result
//     })
// }
