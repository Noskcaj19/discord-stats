use actix_web::{web, App, HttpServer, Responder};
use serenity::prelude::*;
use std::sync::Arc;
use std::thread;

mod store;
use store::StatsStore;

mod event_handler;

fn msg_count(stats: web::Data<Arc<StatsStore>>) -> impl Responder {
    match stats.get_msg_count() {
        Ok(count) => format!(r#"{{"count": {}}}"#, count),
        Err(_) => {
            eprintln!("Error getting message count");
            r#"{{"count": null}}"#.to_owned()
        }
    }
}

fn get_channels(stats: web::Data<Arc<StatsStore>>) -> impl Responder {
    match stats.get_channels() {
        Ok(channels) => web::Json(channels),
        Err(_) => {
            eprintln!("Error getting channels");
            web::Json(vec![])
        }
    }
}

fn get_guilds(stats: web::Data<Arc<StatsStore>>) -> impl Responder {
    match stats.get_guilds() {
        Ok(guilds) => web::Json(guilds),
        Err(_) => {
            eprintln!("Error getting guilds");
            web::Json(vec![])
        }
    }
}

fn main() {
    let stats = match StatsStore::new() {
        Ok(conn) => Arc::new(conn),
        Err(_) => {
            eprintln!("Unable to construct tables. aborting");
            std::process::exit(0);
        }
    };

    // start discord client
    let dc_stats = stats.clone();
    thread::spawn(|| {
        let token = std::env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN not found");

        let mut client = Client::new(&token, event_handler::Handler::with_store(dc_stats))
            .expect("Error creating client");

        if let Err(why) = client.start() {
            eprintln!("Client error: {:?}", why);
        }
    });

    println!("Starting webserver");
    // start web server
    HttpServer::new(move || {
        App::new()
            .data(stats.clone())
            .service(web::resource("/api/count").route(web::get().to(msg_count)))
            .service(web::resource("/api/channels").route(web::get().to(get_channels)))
            .service(web::resource("/api/guilds").route(web::get().to(get_guilds)))
    })
    .bind("127.0.0.1:8080")
    .expect("Unable to bind webserver to port 8080")
    .run()
    .expect("Error starting webserver");
}
