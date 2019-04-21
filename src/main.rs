use serenity::prelude::*;

mod store;
use store::StatsStore;
mod event_handler;

fn main() {
    let token = std::env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN not found");

    let stats = match StatsStore::new() {
        Ok(conn) => conn,
        Err(_) => {
            eprintln!("Unable to construct tables. aborting");
            std::process::exit(0);
        }
    };

    let mut client = Client::new(&token, event_handler::Handler::with_store(stats))
        .expect("Error creating client");

    if let Err(why) = client.start() {
        eprintln!("Client error: {:?}", why);
    }
}
