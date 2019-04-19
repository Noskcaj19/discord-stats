use serenity::{model::gateway::Ready, model::user::User, prelude::Mutex, prelude::*};

mod store;
use store::StatsStore;
mod event_handler;

fn channel_name(channel: serenity::model::channel::Channel) -> String {
    use serenity::model::channel::Channel::*;
    match channel {
        Group(g) => {
            let g = g.read();
            match g.name {
                Some(ref n) => n.clone(),
                None => g
                    .recipients
                    .values()
                    .map(|r| r.read().name.clone())
                    .collect::<Vec<String>>()
                    .join(", "),
            }
        }
        Guild(g) => g.read().name.clone(),
        Private(p) => p.read().name(),
        Category(c) => c.read().name.clone(),
    }
}

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
