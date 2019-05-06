use rusqlite::{ToSql, NO_PARAMS};
use serenity::model::event::MessageUpdateEvent;
use serenity::model::id::{ChannelId, GuildId, MessageId, UserId};
use serenity::{model::channel::Message, prelude::Mutex};
use std::cell::RefCell;
use std::path::Path;
use std::sync::Arc;

use crate::error::StoreError;

pub struct StatsStore {
    conn: Arc<Mutex<rusqlite::Connection>>,
    current_user: Mutex<RefCell<Option<UserId>>>,
}

#[derive(serde_derive::Serialize, Debug, PartialEq, Eq, Hash)]
pub struct Channel {
    pub channel_id: ChannelId,
    pub guild_id: Option<GuildId>,
}

#[derive(Debug)]
pub struct StoreMessage {
    pub message_id: MessageId,
    pub time: i64,
    pub content: String,
    pub channel_id: ChannelId,
    pub guild_id: Option<GuildId>,
    pub author_id: UserId,
}

impl StatsStore {
    pub fn new(path: &Path) -> Result<StatsStore, StoreError> {
        Ok(StatsStore {
            conn: Arc::new(Mutex::new(StatsStore::setup_connection(path)?)),
            current_user: Mutex::new(RefCell::new(None)),
        })
    }

    fn setup_connection(path: &Path) -> Result<rusqlite::Connection, StoreError> {
        let conn = rusqlite::Connection::open(path)?;

        conn.execute(CREATE_MSGS_TABLE_SQL, NO_PARAMS)?;
        conn.execute(CREATE_EDITS_TABLE_SQL, NO_PARAMS)?;
        conn.execute(CREATE_DELETIONS_TABLE_SQL, NO_PARAMS)?;
        Ok(conn)
    }

    pub fn set_current_user(&self, user_id: UserId) {
        *self.current_user.lock().get_mut() = Some(user_id)
    }

    pub fn insert_msg(&self, msg: &Message) -> Result<usize, StoreError> {
        // language=sql
        let query = "
        INSERT INTO main.Messages
        (MessageId, Time, Content, ChannelId, GuildId, AuthorId)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)";

        let data = &[
            &(msg.id.0.to_string()) as &ToSql,
            &msg.timestamp.timestamp(),
            &msg.content,
            &(msg.channel_id.0.to_string()),
            &msg.guild_id.map(|x| x.0.to_string()),
            &(msg.author.id.0.to_string()),
        ];

        Ok(self.conn.lock().execute(query, data)?)
    }

    pub fn insert_edit(&self, update: &MessageUpdateEvent) -> Result<(), StoreError> {
        // language=sql
        let query = "
        SELECT EditId, Times, EditContents FROM Edits WHERE MessageId = ?";

        let q: rusqlite::Result<(i64, String, String)> =
            self.conn
                .lock()
                .query_row(query, &[update.id.0.to_string()], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                });
        match q {
            Ok((edit_id, ref time, ref content)) => {
                let mut times: Vec<i64> = serde_json::from_str(time)?;
                let mut edits: Vec<String> = serde_json::from_str(content)?;

                if let Some(ref timestamp) = update.timestamp {
                    times.push(timestamp.timestamp())
                }
                if let Some(ref new_content) = update.content {
                    edits.push(new_content.clone())
                }

                // Update existing row
                // language=sql
                let query = "
                UPDATE Edits SET Times = ?, EditContents = ? WHERE EditId = ?";

                let data = &[
                    &serde_json::to_string(&times).unwrap() as &ToSql,
                    &serde_json::to_string(&edits).unwrap(),
                    &edit_id,
                ];
                self.conn.lock().execute(query, data)?;
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                // Insert new edit row
                let time = update.edited_timestamp.map(|t| t.timestamp());
                let serialized_time = serde_json::to_string(&vec![time]).unwrap();
                let content =
                    serde_json::to_string(&update.content.as_ref().map(|c| vec![c.clone()]))
                        .unwrap();

                // language=sql
                let query = "
                INSERT INTO Edits (MessageId, ChannelId, Times, EditContents)
                VALUES (?1, ?2, ?3, ?4)";

                let data = &[
                    &update.id.0.to_string() as &ToSql,
                    &update.channel_id.0.to_string(),
                    &serialized_time,
                    &content,
                ];

                self.conn.lock().execute(query, data)?;
            }
            err @ Err(_) => {
                err?;
            }
        };
        Ok(())
    }

    pub fn insert_deletion(
        &self,
        channel_id: ChannelId,
        message_id: MessageId,
    ) -> Result<(), StoreError> {
        // language=sql
        let query = "
        INSERT into Deletions (MessageId, ChannelId, Time)
        VALUES (?1, ?2, ?3)";

        let data = &[
            &message_id.0.to_string() as &ToSql,
            &channel_id.0.to_string(),
            &chrono::offset::Utc::now().timestamp(),
        ];

        self.conn
            .lock()
            .execute(query, data)
            .map(|_| ())
            .map_err(Into::into)
    }

    pub fn get_message_with_channel_id(
        &self,
        channel_id: ChannelId,
        message_id: MessageId,
    ) -> Result<StoreMessage, StoreError> {
        // language=sql
        let query = "
        SELECT MessageId, Time, Content, ChannelId, GuildId, AuthorId
        FROM Messages
        WHERE ChannelId = ?1
        AND MessageId = ?2
        ";

        let conn = self.conn.lock();
        // TODO: figure out error handling here
        conn.query_row(
            query,
            &[
                &channel_id.0.to_string() as &ToSql,
                &message_id.0.to_string(),
            ],
            |row| {
                let message_id: MessageId = row
                    .get::<_, String>(0)?
                    .parse::<u64>()
                    .expect("invalid message_id in db")
                    .into();
                let channel_id: ChannelId = row
                    .get::<_, String>(3)?
                    .parse()
                    .expect("invalid channel_id in db");
                let guild_id: Option<GuildId> = row
                    .get::<_, Option<String>>(4)?
                    .map(|g| GuildId(g.parse().expect("invalid guild_id in db")));
                let author_id: UserId = row
                    .get::<_, String>(5)?
                    .parse::<u64>()
                    .expect("invalid author_id in db")
                    .into();
                Ok(StoreMessage {
                    message_id,
                    time: row.get(1)?,
                    content: row.get(2)?,
                    channel_id,
                    guild_id,
                    author_id,
                })
            },
        )
        .map_err(Into::into)
    }

    pub fn get_msg_count(&self) -> Result<i64, StoreError> {
        let query = "SELECT COUNT(*) FROM Messages";
        Ok(self
            .conn
            .lock()
            .query_row(query, NO_PARAMS, |row| row.get(0))?)
    }

    pub fn get_user_msg_count(&self) -> Result<i64, StoreError> {
        // language=sql
        let query = "SELECT COUNT(*)
        FROM Messages
        WHERE AuthorId = ?";

        let id = self
            .current_user
            .lock()
            .borrow()
            .unwrap_or(UserId(0))
            .0
            .to_string();

        Ok(self.conn.lock().query_row(query, &[id], |row| row.get(0))?)
    }

    pub fn get_user_msgs_per_day(&self) -> Result<Vec<(String, i64, i64)>, StoreError> {
        // language=sql
        let query = "
        SELECT DATE('now', '-7 days')   date_limit,
               DATE(Time, 'unixepoch')  msg_date,
               SUM(GuildId IS NOT NULl) msg_count,
               SUM(GuildId ISNULL)      priv_msg_count
        From Messages
        WHERE AuthorId = ? AND msg_date > date_limit
        GROUP BY msg_date
        ORDER BY msg_date DESC";

        let conn = self.conn.lock();
        let mut stmt = conn.prepare(query)?;

        let id = self
            .current_user
            .lock()
            .borrow()
            .unwrap_or(UserId(0))
            .0
            .to_string();

        stmt.query_map(&[id], |row| Ok((row.get(1)?, row.get(2)?, row.get(3)?)))
            .map(|rows| rows.flatten().collect::<Vec<_>>())
            .map_err(Into::into)
    }

    pub fn get_total_msgs_per_day(&self) -> Result<Vec<(String, i64, i64)>, StoreError> {
        // language=sql
        let query = "
        SELECT DATE(Time, 'unixepoch') msg_date, SUM(GuildId IS NOT NULl) msg_count, SUM(GuildId ISNULL)
        From Messages
        GROUP BY msg_date
        ORDER BY msg_date ASC";

        let conn = self.conn.lock();
        let mut stmt = conn.prepare(query)?;

        stmt.query_map(NO_PARAMS, |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .map(|rows| rows.flatten().collect::<Vec<_>>())
            .map_err(Into::into)
    }

    pub fn get_edit_count(&self) -> Result<i64, StoreError> {
        //language=sql
        let query = "
        SELECT IFNULL(SUM(json_array_length(EditContents)), 0) FROM Edits";

        self.conn
            .lock()
            .query_row(query, NO_PARAMS, |row| row.get(0))
            .map_err(Into::into)
    }

    pub fn get_channels(&self) -> Result<Vec<Channel>, StoreError> {
        // language=sql
        let query = "SELECT DISTINCT ChannelId, GuildId FROM Messages";

        let conn = self.conn.lock();
        let mut stmt = conn.prepare(query)?;

        // TODO: figure out error handling here
        stmt.query_map(NO_PARAMS, |row| {
            Ok(Channel {
                channel_id: row
                    .get::<_, String>(0)?
                    .parse()
                    .expect("invalid channel_id in db"),
                guild_id: row
                    .get::<_, Option<String>>(1)?
                    .map(|g| GuildId(g.parse().expect("invalid guild_id in db"))),
            })
        })
        .map(|rows| rows.flatten().collect::<Vec<_>>())
        .map_err(Into::into)
    }

    pub fn get_guilds(&self) -> Result<Vec<GuildId>, StoreError> {
        // language=sql
        let query = "SELECT DISTINCT GuildId FROM Messages";

        let conn = self.conn.lock();
        let mut stmt = conn.prepare(query)?;

        stmt.query_map(NO_PARAMS, |row| row.get::<_, Option<String>>(0))
            .map(|rows| {
                let mut out: Vec<Option<u64>> = Vec::new();

                for r in rows {
                    if let Ok(i) = r {
                        out.push(i.map(|i| i.parse().expect("invalid guild_id in db")));
                    }
                }

                out.iter().flatten().map(|&g| g.into()).collect()
            })
            .map_err(Into::into)
    }
}

// language=sql
const CREATE_MSGS_TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS Messages
(
    EventId    INTEGER PRIMARY KEY,
    MessageId  TEXT,
    Time       INTEGER,
    Content    TEXT,
    ChannelId  TEXT,
    GuildId    TEXT,
    AuthorId   TEXT,
    UNIQUE (MessageId, ChannelId)
)";

// language=sql
const CREATE_EDITS_TABLE_SQL: &str = "
CREATE TABLE IF NOT EXISTS Edits
(
    EditId          INTEGER PRIMARY KEY,
    MessageId       TEXT,
    ChannelId       TEXT,
    Times           TEXT,
    EditContents    TEXT,
    UNIQUE (MessageId, ChannelId)
)";

// language=sql
const CREATE_DELETIONS_TABLE_SQL: &str = "
CREATE TABLE IF NOT EXISTS Deletions
(
    DeleteId    INTEGER PRIMARY KEY,
    MessageId   TEXT,
    ChannelId   TEXT,
    Time        INTEGER,
    UNIQUE (MessageId, ChannelId)
)
";
