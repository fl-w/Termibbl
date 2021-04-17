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

/// A Skribbl.io-alike for the terminal
#[derive(FromArgs)]
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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
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

    Ok(())
}
