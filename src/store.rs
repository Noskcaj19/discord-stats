use rusqlite::{Connection as SqliteConnection, ToSql, NO_PARAMS};
use serenity::model::event::MessageUpdateEvent;
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

        conn.execute(CREATE_MSGS_TABLE_SQL, NO_PARAMS)?;
        conn.execute(CREATE_EDITS_TABLE_SQL, NO_PARAMS)?;
        Ok(conn)
    }

    pub fn insert_msg(&self, msg: &Message) {
        let data = &[
            &msg.id.to_string() as &ToSql,
            &msg.timestamp.to_rfc3339(),
            &msg.content,
            &msg.channel_id.to_string(),
            &msg.guild_id.map(|x| x.to_string()),
            &msg.author.id.to_string(),
        ];
        if let Err(e) = self.conn.lock().execute(INSERT_MSG_SQL, data) {
            eprintln!("Failed to insert message: {}", e);
        }
    }

    pub fn insert_edit(&self, update: &MessageUpdateEvent) {
        let q: rusqlite::Result<(i64, String)> = self.conn.lock().query_row(
            GET_EDIT_ID_CONTENT_BY_ID,
            &[&update.id.to_string() as &ToSql],
            |row| Ok((row.get(0)?, row.get(1)?)),
        );
        match q {
            Ok((edit_id, content)) => {
                let mut edits: Vec<String> = match serde_json::from_str(&content) {
                    Ok(v) => v,
                    Err(e) => {
                        println!("Error deserializing event content: {:#?}", e);
                        return;
                    }
                };
                if let Some(ref new_content) = update.content {
                    edits.push(new_content.clone())
                }
                // Update existing row
                let data = &[&serde_json::to_string(&edits).unwrap() as &ToSql, &edit_id];
                if let Err(e) = self.conn.lock().execute(UPDATE_EDIT_SQL, data) {
                    eprintln!("Failed to update edit: {}", e);
                }
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                // Insert new edit row
                let data = &[
                    &update.id.to_string() as &ToSql,
                    &update.channel_id.to_string(),
                    &update.timestamp.map(|t| t.to_rfc3339()),
                    &serde_json::to_string(&update.content.as_ref().map(|c| vec![c.clone()]))
                        .unwrap(),
                ];
                if let Err(e) = self.conn.lock().execute(INSERT_EDIT_SQL, data) {
                    eprintln!("Failed to insert edit: {}", e);
                }
            }
            Err(e) => {
                println!("Error fetching edit for message {:#?}", e);
            }
        };
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

// language=sql
const CREATE_MSGS_TABLE_SQL: &'static str = r#"CREATE TABLE IF NOT EXISTS Messages
(
    EventId    INTEGER PRIMARY KEY,
    MessageId  TEXT,
    Time       TEXT,
    Content    TEXT,
    ChannelId  TEXT,
    GuildId    TEXT,
    AuthorId   TEXT
);"#;

// language=sql
const CREATE_EDITS_TABLE_SQL: &'static str = r"
CREATE TABLE IF NOT EXISTS Edits
(
    EditId          INTEGER PRIMARY KEY,
    MessageId       TEXT,
    ChannelId       TEXT,
    Time            TEXT,
    EditContent     TEXT
)";

// language=sql
const INSERT_MSG_SQL: &'static str = r#"INSERT INTO main.Messages
    (MessageId, Time, Content, ChannelId, GuildId, AuthorId)
VALUES (?1, ?2, ?3, ?4, ?5, ?6);"#;

// language=sql
const GET_MSG_COUNT_SQL: &'static str = r#"SELECT COUNT(*) FROM Messages"#;

// language=sql
const GET_CHANNELS_SQL: &'static str = "SELECT DISTINCT ChannelId, GuildId FROM Messages";

// language=sql
const GET_GUILDS_SQL: &'static str = "SELECT DISTINCT GuildId FROM Messages";

// language=sql
const INSERT_EDIT_SQL: &'static str = "
INSERT INTO Edits (MessageId, ChannelId, Time, EditContent)
VALUES (?1, ?2, ?3, ?4)";

// Gets the edit id and content of an edit by its message id
// language=sql
const GET_EDIT_ID_CONTENT_BY_ID: &'static str = "
SELECT EditId, EditContent FROM Edits WHERE MessageId = ?
";

// language=sql
const UPDATE_EDIT_SQL: &'static str = "
UPDATE Edits SET EditContent = ? WHERE EditId = ?
";
