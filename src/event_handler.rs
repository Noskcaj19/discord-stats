use serenity::{model::prelude::*, prelude::*};
use std::cell::RefCell;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

use crate::store::StatsStore;

pub struct Handler {
    store: Arc<StatsStore>,
    user: Mutex<RefCell<Option<User>>>,
    additional_channels: Vec<(Option<GuildId>, ChannelId)>,
}

impl Handler {
    pub fn new(
        store: Arc<StatsStore>,
        additional_channels: Vec<(Option<GuildId>, ChannelId)>,
    ) -> Handler {
        Handler {
            store,
            user: Mutex::new(RefCell::new(None)),
            additional_channels,
        }
    }
}

impl EventHandler for Handler {
    fn message(&self, _ctx: Context, m: Message) {
        if let Some(ref user) = *self.user.lock().borrow() {
            if user == &m.author
                || self
                    .additional_channels
                    .contains(&(m.guild_id, m.channel_id))
            {
                self.store.insert_msg(&m)
            }
        }
    }

    fn message_delete(&self, _ctx: Context, channel_id: ChannelId, message_id: MessageId) {
        if let Ok(msg) = self
            .store
            .get_message_with_channel_id(channel_id, message_id)
        {
            if let Some(ref user) = *self.user.lock().borrow() {
                if user.id == msg.author_id
                    || self
                        .additional_channels
                        .contains(&(msg.guild_id, msg.channel_id))
                {
                    self.store.insert_deletion(channel_id, message_id);
                }
            }
        }
    }

    fn message_delete_bulk(
        &self,
        _ctx: Context,
        channel_id: ChannelId,
        message_ids: Vec<MessageId>,
    ) {
        for message_id in message_ids {
            if let Ok(msg) = self
                .store
                .get_message_with_channel_id(channel_id, message_id)
            {
                if let Some(ref user) = *self.user.lock().borrow() {
                    if user.id == msg.author_id
                        || self
                            .additional_channels
                            .contains(&(msg.guild_id, msg.channel_id))
                    {
                        self.store.insert_deletion(channel_id, message_id);
                    }
                }
            }
        }
    }

    fn message_update(
        &self,
        _ctx: Context,
        _old: Option<Message>,
        _new: Option<Message>,
        update: MessageUpdateEvent,
    ) {
        if let Some(ref user) = *self.user.lock().borrow() {
            if let Some(ref author) = update.author {
                if user == author {
                    self.store.insert_edit(&update)
                }
            }
        }
    }

    fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected", ready.user.name);
        *self.user.lock().borrow_mut() = Some(ready.user.into());
        ctx.set_presence(None, serenity::model::user::OnlineStatus::Offline);
    }
}

pub struct OneshotData {
    pub context: Context,
    pub ready: Ready,
}

pub struct OneshotHandler {
    tx: Arc<Mutex<Sender<OneshotData>>>,
}

impl OneshotHandler {
    pub fn new() -> (Receiver<OneshotData>, OneshotHandler) {
        let (tx, rx) = std::sync::mpsc::channel();

        let tx = Arc::new(Mutex::new(tx));
        (rx, OneshotHandler { tx })
    }
}

impl EventHandler for OneshotHandler {
    fn ready(&self, context: Context, ready: Ready) {
        let _ = self.tx.lock().send(OneshotData { context, ready });
    }
}
