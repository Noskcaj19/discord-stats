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

    fn should_handle(
        &self,
        user_id: UserId,
        guild_id: Option<GuildId>,
        channel_id: ChannelId,
    ) -> bool {
        if let Some(ref current_user) = *self.user.lock().borrow() {
            if current_user.id == user_id
                || self.additional_channels.contains(&(guild_id, channel_id))
            {
                return true;
            }
        }
        false
    }

    #[allow(dead_code)]
    fn should_handle_no_guild(&self, user_id: UserId) -> bool {
        if let Some(ref current_user) = *self.user.lock().borrow() {
            if current_user.id == user_id {
                return true;
            }
        }
        false
    }
}

impl EventHandler for Handler {
    fn message(&self, _ctx: Context, m: Message) {
        if self.should_handle(m.author.id, m.guild_id, m.channel_id) {
            self.store.insert_msg(&m)
        }
    }

    fn message_delete(&self, _ctx: Context, channel_id: ChannelId, message_id: MessageId) {
        if let Ok(msg) = self
            .store
            .get_message_with_channel_id(channel_id, message_id)
        {
            if self.should_handle(msg.author_id, msg.guild_id, msg.channel_id) {
                self.store.insert_deletion(channel_id, message_id);
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
                if self.should_handle(msg.author_id, msg.guild_id, msg.channel_id) {
                    self.store.insert_deletion(channel_id, message_id);
                }
            }
        }
    }

    fn message_update(
        &self,
        _ctx: Context,
        old: Option<Message>,
        new: Option<Message>,
        update: MessageUpdateEvent,
    ) {
        if let Some(ref author) = update.author {
            let msg = new.or(old);
            let guild = msg.as_ref().and_then(|msg| msg.guild_id);
            if self.should_handle(author.id, guild, update.channel_id) {
                self.store.insert_edit(&update)
            }
        }
    }

    fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected", ready.user.name);
        self.store.set_current_user(ready.user.id);

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
        {
            let mut ctx_lock = context.cache.write();
            for (&c_id, ch) in &ready.private_channels {
                if let Some(private) = ch.clone().private() {
                    ctx_lock.private_channels.insert(c_id, private);
                }
            }
        }
        let _ = self.tx.lock().send(OneshotData { context, ready });
    }
}
