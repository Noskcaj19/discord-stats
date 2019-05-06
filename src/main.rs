use iron::{Chain, Iron};
use persistent::Read;
use router::router;
use serde_derive::{Deserialize, Serialize};
use serenity::model::id::{ChannelId, GuildId};
use serenity::prelude::*;
use std::collections::HashSet;
use std::fs::DirBuilder;
use std::sync::Arc;
use std::thread;

mod store;
use store::StatsStore;

mod scan;

mod api;
mod event_handler;
use event_handler::OneshotData;

mod error;
use error::ConfigError;

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
    pub fn load() -> Result<Config, ConfigError> {
        let config_path = Config::config_path().ok_or(ConfigError::NoHome)?;

        Ok(if !config_path.exists() {
            DirBuilder::new()
                .recursive(true)
                .create(config_path.parent().ok_or(ConfigError::NoParent)?)?;
            let conf = Config::default();
            std::fs::write(
                &config_path,
                toml::to_string(&conf).expect("configuration is serializable"),
            )?;
            conf
        } else {
            let config_str = std::fs::read_to_string(config_path)?;
            toml::from_str(&config_str)?
        })
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let config_path = Config::config_path().ok_or(ConfigError::NoHome)?;

        std::fs::write(
            config_path,
            toml::to_string(self).expect("configuration is serializable"),
        )?;
        Ok(())
    }

    pub fn tracked_channels(&self) -> Result<Vec<(Option<GuildId>, ChannelId)>, ConfigError> {
        let mut out = Vec::new();
        for channel in &self.tracked_channels {
            let chan = if channel.contains('|') {
                let mut split_item = channel.split('|');
                let guild = split_item.next().ok_or(ConfigError::InvalidGuildFormat)?;
                let channel = split_item.next().ok_or(ConfigError::InvalidChannelFormat)?;
                (
                    Some(GuildId(
                        guild
                            .parse::<u64>()
                            .map_err(|_| ConfigError::InvalidChannelFormat)?,
                    )),
                    channel
                        .parse()
                        .map_err(|_| ConfigError::InvalidChannelFormat)?,
                )
            } else {
                (
                    None,
                    channel
                        .parse()
                        .map_err(|_| ConfigError::InvalidChannelFormat)?,
                )
            };
            out.push(chan);
        }
        Ok(out)
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
    let mut config = match Config::load() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error loading configuration:\n{:?}", e);
            std::process::exit(1)
        }
    };
    let db_path = match Config::data_root() {
        Some(data) => data.join("store.sqlite3"),
        None => {
            eprintln!("Unable to get users config dir");
            std::process::exit(2)
        }
    };

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
        .subcommand(
            SubCommand::with_name("fetch-history")
                .about("Add previously sent messages to the log")
                .arg(
                    Arg::with_name("max-count")
                        .help("Maximum amount of messages to search")
                        .long("max-count")
                        .default_value("500")
                        .takes_value(true)
                        .short("c"),
                ),
        )
        .get_matches();

    if let Some(store_token) = matches.subcommand_matches("store-token") {
        let token = store_token
            .value_of("token")
            .expect("token is a required field");
        config.discord_token = token.to_owned();

        if let Err(e) = config.save() {
            eprintln!("An error occured saving the configuration file:\n{:?}", e);
        } else {
            println!("Successfully saved discord token");
        }

        return;
    }

    let token = std::env::var("DISCORD_TOKEN").unwrap_or(config.discord_token.clone());
    if token.is_empty() || serenity::client::validate_token(&token).is_err() {
        eprintln!("Empty or invalid token, please set it by running `discord-statistics token $DISCORD_TOKEN`\nexiting");
        return;
    }

    if let Some(track) = matches.subcommand_matches("track") {
        // Name of the private channel user or guild
        let group_name = track
            .value_of("group-name")
            .expect("group-name is a required field");

        let data = get_oneshot_data(&token);

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
                if let Err(e) = config.save() {
                    eprintln!("An error occured saving the configuration file:\n{:?}", e);
                }
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

    if let Some(fetch) = matches.subcommand_matches("fetch-history") {
        let max_count: u64 = match fetch.value_of("max-count").unwrap_or("500").parse() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("max-count must be an integer");
                return;
            }
        };
        let data = get_oneshot_data(&token);

        let mut channels_to_scan = HashSet::new();
        let tracked_channels = match config.tracked_channels() {
            Ok(channels) => channels,
            Err(e) => {
                eprintln!("Error loading channels:\n{:?}", e);
                std::process::exit(2);
            }
        };
        for (guild_id, channel_id) in tracked_channels {
            channels_to_scan.insert(store::Channel {
                guild_id,
                channel_id,
            });
        }
        if let Ok(logged_channels) = stats.get_channels() {
            channels_to_scan.extend(logged_channels)
        }

        println!("Scanning channels");
        for channel in &channels_to_scan {
            println!(
                " {}",
                channel
                    .channel_id
                    .name(&data.context)
                    .unwrap_or_else(|| channel.channel_id.0.to_string())
            )
        }

        scan::MessageScanner { data, store: stats }.scan_messages(&channels_to_scan, max_count);

        return;
    }

    // start web server
    let http_stats = stats.clone();
    thread::spawn(|| {
        println!("Starting webserver");

        let router = router! {
            api_total_msg_count_per_day: get "/api/total_msg_count_per_day" => api::total_msg_count_per_day,
            api_user_msg_count_per_day: get "/api/user_msg_count_per_day" => api::msg_count_per_day,
            api_total_msg_count: get "/api/total_msg_count" => api::total_msg_count,
            api_edit_count: get "/api/edit_count" => api::edit_count,
            api_channels: get "/api/channels" => api::get_channels,
            api_msg_count: get "/api/msg_count" => api::msg_count,
            dashboard_js: get "/index.js" => api::dashboard_js,
            api_guilds: get "/api/guilds" => api::get_guilds,
            dashboard_g: get "/*" => api::dashboard,
            dashboard: get "/" => api::dashboard,
        };

        let mut chain = Chain::new(router);
        chain.link(Read::<api::Stats>::both(http_stats));
        let server = Iron::new(chain).http("localhost:8080");
        if let Err(e) = server {
            eprintln!("Unable to create http servere on port 8080: {:?}", e)
        }
    });

    let tracked_channels = match config.tracked_channels() {
        Ok(channels) => channels,
        Err(e) => {
            eprintln!("Config contains invalid tracked channels:\n{:?}", e);
            std::process::exit(2)
        }
    };

    // start discord client
    let handler = event_handler::Handler::new(stats.clone(), tracked_channels);
    let mut client = match Client::new(&token, handler) {
        Ok(client) => client,
        Err(e) => {
            eprintln!("Error starting discord client: {:#?}", e);
            std::process::exit(3)
        }
    };

    if let Err(why) = client.start() {
        eprintln!("Unable to connect to discord: {:?}", why);
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

fn get_oneshot_data(token: &str) -> OneshotData {
    let (rx, handler) = event_handler::OneshotHandler::new();
    let mut client = match Client::new(&token, handler) {
        Ok(client) => client,
        Err(e) => {
            eprintln!("Error starting discord client: {:#?}", e);
            std::process::exit(3);
        }
    };
    thread::spawn(move || {
        if let Err(e) = client.start() {
            eprintln!("Unable to connect to discord: {:#?}", e);
        }
    });
    rx.recv().expect("event handler should not panic")
}
