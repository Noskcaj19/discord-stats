use rusqlite::{Connection as SqliteConnection, ToSql};
use serenity::{model::channel::Message, prelude::Mutex};
use std::sync::Arc;

pub struct StatsStore {
    conn: Arc<Mutex<SqliteConnection>>,
}

impl StatsStore {
    pub fn new() -> Result<StatsStore, rusqlite::Error> {
        Ok(StatsStore {
            conn: Arc::new(Mutex::new(StatsStore::setup_connection()?)),
        })
    }

    fn setup_connection() -> Result<SqliteConnection, rusqlite::Error> {
        let conn = SqliteConnection::open("db.sqlite3").expect("Unable to open databasse");

        conn.execute(CREATE_MSGS_TABLE_SQL, rusqlite::NO_PARAMS)
            .map(|_| conn)
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
