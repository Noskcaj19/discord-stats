use iron::{Chain, Iron};
use persistent::Read;
use router::router;
use serde_derive::{Deserialize, Serialize};
use serenity::model::id::{ChannelId, GuildId};
use serenity::prelude::*;
use std::fs::DirBuilder;
use std::sync::Arc;
use std::thread;

mod store;
use store::StatsStore;

mod api;
mod event_handler;
use event_handler::OneshotData;

#[derive(Serialize, Deserialize)]
struct Config {
    discord_token: String,
    tracked_channels: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            discord_token: String::new(),
            tracked_channels: Vec::new(),
        }
    }
}

impl Config {
    pub fn load() -> Config {
        let config_path =
            Config::config_path().expect("Unable to find users home dir or config path");

        if !config_path.exists() {
            DirBuilder::new()
                .recursive(true)
                .create(config_path.parent().expect("Config path has no parent?"))
                .expect(&format!(
                    "Unable to create config folder at {:?}",
                    config_path
                ));
            let conf = Config::default();
            std::fs::write(&config_path, toml::to_string(&conf).unwrap()).expect(&format!(
                "Unable to write default config values to {:?}",
                &config_path
            ));
            conf
        } else {
            let config_str =
                std::fs::read_to_string(config_path).expect("Unable to read config file");
            toml::from_str(&config_str).expect("Invalid config file")
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let config_path =
            Config::config_path().expect("Unable to find users home dir or config path");

        std::fs::write(config_path, toml::to_string(self).unwrap())
    }

    pub fn tracked_channels(&self) -> Vec<(Option<GuildId>, ChannelId)> {
        self.tracked_channels
            .iter()
            .map(|i| {
                if i.contains('|') {
                    let mut split_item = i.split('|');
                    let guild = split_item.next().expect("Invalid tracked channel");
                    let channel = split_item.next().expect("Invalid tracked guild");
                    (
                        Some(GuildId(
                            guild.parse::<u64>().expect("Invalid tracked channel"),
                        )),
                        channel.parse().expect("Invalid tracked guild"),
                    )
                } else {
                    (None, i.parse().unwrap())
                }
            })
            .collect::<Vec<_>>()
    }

    pub fn config_path() -> Option<std::path::PathBuf> {
        Config::data_root().map(|h| h.join("config.toml"))
    }

    #[cfg(target_os = "macos")]
    pub fn data_root() -> Option<std::path::PathBuf> {
        dirs::home_dir().map(|h| h.join(".config/discord-statistics/"))
    }

    #[cfg(not(target_os = "macos"))]
    pub fn data_root() -> Option<std::path::PathBuf> {
        dirs::config_dir().map(|h| h.join("discord-statistics/"))
    }
}

fn main() {
    let mut config = Config::load();
    let db_path = Config::data_root().unwrap().join("store.sqlite3");

    use clap::{App, Arg, SubCommand};
    let matches = App::new("Discord statistics")
        .author("Noskcaj19")
        .subcommand(
            SubCommand::with_name("token")
                .about("Store your Discord token")
                .arg(
                    Arg::with_name("token")
                        .required(true)
                        .help("Discord user token"),
                ),
        )
        .subcommand(
            SubCommand::with_name("track")
                .about("Start tracking a channel")
                .arg(
                    Arg::with_name("group-name")
                        .required(true)
                        .help("Guild name or private channel user"),
                )
                .arg(
                    Arg::with_name("channel-name")
                        .required(false)
                        .help("Channel name if guild is provided"),
                ),
        )
        .get_matches();

    if let Some(store_token) = matches.subcommand_matches("store-token") {
        let token = store_token.value_of("token").unwrap();
        config.discord_token = token.to_owned();

        config.save().expect("Unable to save configuration");

        println!("Successfully saved discord token");
        return;
    }

    let token = std::env::var("DISCORD_TOKEN").unwrap_or(config.discord_token.clone());
    if token.is_empty() || serenity::client::validate_token(&token).is_err() {
        eprintln!("Empty or invalid token, please set it with `discord-statistics token $DISCORD_TOKEN`, exiting");
        return;
    }

    if let Some(track) = matches.subcommand_matches("track") {
        // Name of the private channel user or guild
        let group_name = track.value_of("group-name").unwrap();
        let (rx, handler) = event_handler::OneshotHandler::new();
        let mut client = Client::new(&token, handler).expect("Unable to create client");
        thread::spawn(move || {
            client.start().expect("Unable starting discord client");
        });
        let data = rx.recv().expect("Unable to retrieve discord data");

        let id_str = if let Some(channel_name) = track.value_of("channel-name") {
            resolve_guild_channel_names(&data, group_name, channel_name)
                .map(|(gid, cid)| format!("{}|{}", gid.0, cid.0))
        } else {
            resolve_private_channel(&data, group_name).map(|id| id.0.to_string())
        };
        match id_str {
            Some(id_str) => {
                config.tracked_channels.push(id_str);
                println!("Added channel to tracking list");
                config.save().expect("Unable to save config")
            }
            None => eprintln!("Unable to find a matching channel"),
        }

        return;
    }

    let stats = match StatsStore::new(&db_path) {
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
            api_guilds: get "/api/guilds" => api::get_guilds,
            dashboard_g: get "/*" => api::dashboard,
            dashboard: get "/" => api::dashboard,
        };

        let mut chain = Chain::new(router);
        chain.link(Read::<api::Stats>::both(http_stats));
        let _server = Iron::new(chain).http("localhost:8080").unwrap();
    });

    // start discord client
    let handler = event_handler::Handler::new(stats.clone(), config.tracked_channels());
    let mut client = Client::new(&token, handler).expect("Error creating client");

    if let Err(why) = client.start() {
        eprintln!("Client error: {:?}", why);
    }
}

fn resolve_guild_channel_names(
    data: &OneshotData,
    guild_name: &str,
    channel_name: &str,
) -> Option<(GuildId, ChannelId)> {
    for guild in &data.ready.guilds {
        use serenity::model::guild::GuildStatus::*;
        let (guild_id, name) = match guild {
            OnlinePartialGuild(g) => (g.id, g.name.clone()),
            OnlineGuild(g) => (g.id, g.name.clone()),
            Offline(g) => (
                g.id,
                g.id.to_partial_guild(&data.context.http)
                    .expect("Unable to fetch guild data")
                    .name
                    .clone(),
            ),
            _ => panic!("Unknown guild state"),
        };

        if guild_name.to_lowercase() == name.to_lowercase() {
            let channels = guild_id
                .channels(&data.context.http)
                .expect("Unable to fetch guild channels");

            for (&channel_id, channel) in channels.iter() {
                use serenity::model::channel::ChannelType::*;
                match channel.kind {
                    Text | Private | Group | News => {}
                    _ => continue,
                }

                if channel_name.to_lowercase() == channel.name.to_lowercase() {
                    return Some((guild_id, channel_id));
                }
            }
        }
    }
    None
}

fn resolve_private_channel(data: &OneshotData, user_name: &str) -> Option<ChannelId> {
    for (&id, channel) in data.ready.private_channels.iter() {
        if user_name.to_lowercase() == format!("{}", channel).to_lowercase() {
            return Some(id);
        }
    }
    None
}
