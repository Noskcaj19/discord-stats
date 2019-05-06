use crate::error::StoreError;
use crate::event_handler::OneshotData;
use crate::store;
use crate::store::StatsStore;
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
        let cache = self.data.context.cache.read();
        let http = &self.data.context.http;
        let msg_limit_per_call = max_count.min(100);
        for channel_to_scan in channels {
            if let Some(guild_id) = channel_to_scan.guild_id {
                // Guild Channel
                if let Ok(guild_channels) = guild_id.channels(http) {
                    if let Some(channel) = guild_channels.get(&channel_to_scan.channel_id) {
                        self.scan_channel(
                            max_count,
                            msg_limit_per_call,
                            Some(guild_id),
                            channel.id,
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
                    self.scan_channel(max_count, msg_limit_per_call, None, channel.read().id);
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
    ) {
        let mut searched = 0;
        let mut last_msg = None;
        let http = &self.data.context.http;
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
                    for msg in &mut msgs {
                        // why is this necessary?
                        msg.guild_id = guild_id;
                        match self.store.insert_msg(msg) {
                            Ok(_rows) => {}
                            Err(StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows)) => {}
                            err @ _ => eprintln!("Unable to insert message: {:?}", err),
                        };
                    }
                    last_msg = msgs.last().cloned();
                }
                Err(e) => {
                    eprintln!("Got loop err: {:#?}", e);
                    break;
                }
            };
            searched += msg_limit_per_call;
        }
    }
}
