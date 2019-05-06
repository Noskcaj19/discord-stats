use crate::error::StoreError;
use crate::event_handler::OneshotData;
use crate::store;
use crate::store::StatsStore;
use indicatif::{ProgressBar, ProgressStyle};
use rusqlite::ErrorCode;
use serenity::model::id::GuildId;
use serenity::model::prelude::ChannelId;
use std::collections::HashSet;
use std::sync::Arc;

pub struct MessageScanner {
    pub data: OneshotData,
    pub store: Arc<StatsStore>,
}

impl MessageScanner {
    pub fn scan_messages(&self, channels: &HashSet<store::Channel>, max_count: u64) {
        let channels = channels
            .iter()
            .map(|channel| {
                let channel_name;
                if let Some(guild_id) = channel.guild_id {
                    channel_name = guild_id
                        .channels(&self.data.context.http)
                        .ok()
                        .and_then(|channels| channels.get(&channel.channel_id).cloned())
                        .map(|channel| "#".to_owned() + &channel.name)
                } else {
                    channel_name = channel.channel_id.name(&self.data.context)
                }
                (
                    channel_name.unwrap_or_else(|| channel.channel_id.0.to_string()),
                    channel,
                )
            })
            .collect::<Vec<_>>();

        let cache_clone = Arc::clone(&self.data.context.cache);
        let cache = cache_clone.read();
        let http = &self.data.context.http;
        let msg_limit_per_call = max_count.min(100);
        for (channel_name, channel_to_scan) in channels {
            if let Some(guild_id) = channel_to_scan.guild_id {
                // Guild Channel
                if let Ok(guild_channels) = guild_id.channels(http) {
                    if let Some(channel) = guild_channels.get(&channel_to_scan.channel_id) {
                        self.scan_channel(
                            max_count,
                            msg_limit_per_call,
                            Some(guild_id),
                            channel.id,
                            &channel_name,
                        );
                    } else {
                        eprintln!(
                            "Error getting guild {:?} channel {:?}",
                            guild_id, channel_to_scan.channel_id,
                        )
                    }
                }
            } else {
                // Direct message
                if let Some(channel) = cache.private_channels.get(&channel_to_scan.channel_id) {
                    self.scan_channel(
                        max_count,
                        msg_limit_per_call,
                        None,
                        channel.read().id,
                        &channel_name,
                    );
                } else {
                    eprintln!("Error getting channel {:?}", channel_to_scan.channel_id,)
                }
            }
        }
    }

    fn scan_channel(
        &self,
        max_count: u64,
        msg_limit_per_call: u64,
        guild_id: Option<GuildId>,
        channel: ChannelId,
        channel_name: &str,
    ) {
        let mut searched = 0;
        let mut collected = 0;
        let mut last_msg = None;
        let http = &self.data.context.http;

        let pb = ProgressBar::new(max_count);
        pb.set_style(
            ProgressStyle::default_bar()
                .progress_chars("##-")
                .template(" {msg} {wide_bar} {pos}/{len} "),
        );

        pb.set_message(&channel_name);

        while searched < max_count {
            let msgs = if let Some(last) = last_msg {
                channel.messages(http, |retriever| {
                    retriever.limit(msg_limit_per_call).before(last)
                })
            } else {
                channel.messages(http, |retriever| retriever.limit(msg_limit_per_call))
            };

            match msgs {
                Ok(mut msgs) => {
                    if msgs.is_empty() {
                        break;
                    };
                    collected += msgs.len() as u64;
                    for msg in &mut msgs {
                        // why is this necessary?
                        msg.guild_id = guild_id;
                        // Ignore success and UNIQUE constraint errors
                        match self.store.insert_msg(msg) {
                            Ok(_rows) => {}
                            Err(StoreError::Sqlite(rusqlite::Error::SqliteFailure(e, _)))
                                if e.code == ErrorCode::ConstraintViolation => {}
                            err @ _ => pb.println(format!("Unable to insert message: {:?}", err)),
                        };
                    }
                    last_msg = msgs.last().cloned();
                }
                Err(e) => {
                    pb.println(format!("Error fetching messages: {:#?}", e));
                    break;
                }
            };
            pb.inc(msg_limit_per_call);
            searched += msg_limit_per_call;
        }
        pb.set_length(collected);
        pb.finish();
    }
}
