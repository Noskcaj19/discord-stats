use iron::{Chain, Iron};
use persistent::Read;
use router::router;
use serenity::prelude::*;
use std::sync::Arc;
use std::thread;

mod store;
use store::StatsStore;

mod api;
mod event_handler;

fn main() {
    let token = std::env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN not found");
    let stats = match StatsStore::new() {
        Ok(conn) => Arc::new(conn),
        Err(_) => {
            eprintln!("Unable to construct tables. aborting");
            std::process::exit(0);
        }
    };

    // start web server
    let http_stats = stats.clone();
    thread::spawn(|| {
        println!("Starting webserver");

        let router = router! {
            api_msg_count: get "/api/msg_count" => api::msg_count,
            api_channels: get "/api/channels" => api::get_channels,
            api_guilds: get "/api/guilds" => api::get_guilds
        };

        let mut chain = Chain::new(router);
        chain.link(Read::<api::Stats>::both(http_stats));
        let _server = Iron::new(chain).http("localhost:8080").unwrap();
    });

    // start discord client

    let mut client = Client::new(&token, event_handler::Handler::with_store(stats))
        .expect("Error creating client");

    if let Err(why) = client.start() {
        eprintln!("Client error: {:?}", why);
    }
}
