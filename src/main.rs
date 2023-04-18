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
    Server(server::CliOpts),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli: Opt = argh::from_env();

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

        SubOpt::Server(mut cli) => {
            let port = cli.port;
            let custom_word_list = cli.words.take();
            let default_game_opts = data::GameOpts {
                dimensions: cli.dimensions,
                number_of_rounds: cli.rounds,
                draw_time: cli.draw_time as usize,
                only_custom_words: false,
            };
            server::run(port, default_game_opts, custom_word_list).await?;
        }
    }

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
