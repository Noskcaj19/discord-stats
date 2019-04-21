use rusqlite::{Connection as SqliteConnection, ToSql, NO_PARAMS};
use serenity::{model::channel::Message, prelude::Mutex};
use std::sync::Arc;

pub struct StatsStore {
    conn: Arc<Mutex<SqliteConnection>>,
}

#[derive(serde::Serialize, Debug)]
pub struct Channel {
    pub channel_id: String,
    pub guild_id: Option<String>,
}

impl StatsStore {
    pub fn new() -> Result<StatsStore, rusqlite::Error> {
        Ok(StatsStore {
            conn: Arc::new(Mutex::new(StatsStore::setup_connection()?)),
        })
    }

    fn setup_connection() -> Result<SqliteConnection, rusqlite::Error> {
        let conn = SqliteConnection::open("db.sqlite3").expect("Unable to open databasse");

        conn.execute(CREATE_MSGS_TABLE_SQL, NO_PARAMS).map(|_| conn)
    }

    pub fn insert_msg(&self, msg: &Message) {
        let data = &[
            &msg.id.to_string() as &ToSql,
            &msg.timestamp.to_rfc3339(),
            &msg.content.len().to_string(),
            &msg.channel_id.to_string(),
            &msg.guild_id.map(|x| x.to_string()),
            &msg.author.id.to_string(),
        ];
        if let Err(e) = self.conn.lock().execute(INSERT_MSG_SQL, data) {
            eprintln!("Failed to insert message: {}", e);
        }
    }

    pub fn get_msg_count(&self) -> rusqlite::Result<i32> {
        self.conn
            .lock()
            .query_row(GET_MSG_COUNT_SQL, NO_PARAMS, |row| row.get(0))
    }

    pub fn get_channels(&self) -> rusqlite::Result<Vec<Channel>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(GET_CHANNELS_SQL)?;

        stmt.query_map(NO_PARAMS, |row| {
            Ok(Channel {
                channel_id: row.get(0)?,
                guild_id: row.get(1)?,
            })
        })
        .map(|rows| rows.flatten().collect::<Vec<_>>())
    }

    pub fn get_guilds(&self) -> rusqlite::Result<Vec<String>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(GET_GUILDS_SQL)?;

        stmt.query_map(NO_PARAMS, |row| row.get(0))
            .map(|rows| rows.flatten().collect::<Vec<_>>())
    }
}

const CREATE_MSGS_TABLE_SQL: &'static str = r#"CREATE TABLE IF NOT EXISTS Messages
(
    EventId    INTEGER PRIMARY KEY,
    MessageId  TEXT,
    Time       TEXT,
    MessageLen INTEGER,
    ChannelId  TEXT,
    GuildId    TEXT,
    AuthorId   TEXT
);"#;

const INSERT_MSG_SQL: &'static str = r#"INSERT INTO main.Messages
    ("MessageId", "Time", "MessageLen", "ChannelId", "GuildId", "AuthorId")
VALUES (?1, ?2, ?3, ?4, ?5, ?6);"#;

const GET_MSG_COUNT_SQL: &'static str = r#"SELECT COUNT(*) FROM Messages"#;

const GET_CHANNELS_SQL: &'static str = "SELECT DISTINCT ChannelId, GuildId FROM Messages";

const GET_GUILDS_SQL: &'static str = "SELECT DISTINCT GuildId FROM Messages";
