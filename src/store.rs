use rusqlite::{Connection as SqliteConnection, ToSql, NO_PARAMS};
use serenity::model::event::MessageUpdateEvent;
use serenity::{model::channel::Message, prelude::Mutex};
use std::path::Path;
use std::sync::Arc;

pub struct StatsStore {
    conn: Arc<Mutex<SqliteConnection>>,
}

#[derive(serde_derive::Serialize, Debug)]
pub struct Channel {
    pub channel_id: i64,
    pub guild_id: Option<String>,
}

impl StatsStore {
    pub fn new(path: &Path) -> Result<StatsStore, rusqlite::Error> {
        Ok(StatsStore {
            conn: Arc::new(Mutex::new(StatsStore::setup_connection(path)?)),
        })
    }

    fn setup_connection(path: &Path) -> Result<SqliteConnection, rusqlite::Error> {
        let conn = SqliteConnection::open(path).expect("Unable to open database");

        conn.execute(CREATE_MSGS_TABLE_SQL, NO_PARAMS)?;
        conn.execute(CREATE_EDITS_TABLE_SQL, NO_PARAMS)?;
        Ok(conn)
    }

    pub fn insert_msg(&self, msg: &Message) {
        let data = &[
            &(msg.id.0 as i64) as &ToSql,
            &msg.timestamp.to_rfc3339(),
            &msg.content,
            &(msg.channel_id.0 as i64),
            &msg.guild_id.map(|x| x.0 as i64),
            &(msg.author.id.0 as i64),
        ];
        if let Err(e) = self.conn.lock().execute(INSERT_MSG_SQL, data) {
            eprintln!("Failed to insert message: {}", e);
        }
    }

    pub fn insert_edit(&self, update: &MessageUpdateEvent) {
        let q: rusqlite::Result<(i64, String, String)> =
            self.conn
                .lock()
                .query_row(GET_EDIT_ID_CONTENT_BY_ID, &[update.id.0 as i64], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                });
        match q {
            Ok((edit_id, ref time, ref content)) => {
                let mut times: Vec<String> = match serde_json::from_str(time) {
                    Ok(v) => v,
                    Err(e) => {
                        println!("Error deserializing event times: {:#?}", e);
                        return;
                    }
                };
                let mut edits: Vec<String> = match serde_json::from_str(content) {
                    Ok(v) => v,
                    Err(e) => {
                        println!("Error deserializing event content: {:#?}", e);
                        return;
                    }
                };

                if let Some(ref timestamp) = update.timestamp {
                    times.push(timestamp.to_rfc3339())
                }
                if let Some(ref new_content) = update.content {
                    edits.push(new_content.clone())
                }
                // Update existing row
                let data = &[
                    &serde_json::to_string(&times).unwrap() as &ToSql,
                    &serde_json::to_string(&edits).unwrap(),
                    &edit_id,
                ];
                if let Err(e) = self.conn.lock().execute(UPDATE_EDIT_SQL, data) {
                    eprintln!("Failed to update edit: {}", e);
                }
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                // Insert new edit row
                let time = update.edited_timestamp.map(|t| t.to_rfc3339());
                let serialized_time = serde_json::to_string(&vec![time]).unwrap();
                let content =
                    serde_json::to_string(&update.content.as_ref().map(|c| vec![c.clone()]))
                        .unwrap();
                let data = &[
                    &(update.id.0 as i64) as &ToSql,
                    &(update.channel_id.0 as i64),
                    &serialized_time,
                    &content,
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

    pub fn get_msg_count(&self) -> rusqlite::Result<i64> {
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
const CREATE_MSGS_TABLE_SQL: &str = r#"CREATE TABLE IF NOT EXISTS Messages
(
    EventId    INTEGER PRIMARY KEY,
    MessageId  INTEGER,
    Time       TEXT,
    Content    TEXT,
    ChannelId  INTEGER,
    GuildId    INTEGER,
    AuthorId   INTEGER
);"#;

// language=sql
const CREATE_EDITS_TABLE_SQL: &str = r"
CREATE TABLE IF NOT EXISTS Edits
(
    EditId          INTEGER PRIMARY KEY,
    MessageId       INTEGER,
    ChannelId       INTEGER,
    Times           TEXT,
    EditContents    TEXT
)";

// language=sql
const INSERT_MSG_SQL: &str = r#"INSERT INTO main.Messages
    (MessageId, Time, Content, ChannelId, GuildId, AuthorId)
VALUES (?1, ?2, ?3, ?4, ?5, ?6);"#;

// language=sql
const GET_MSG_COUNT_SQL: &str = r#"SELECT COUNT(*) FROM Messages"#;

// language=sql
const GET_CHANNELS_SQL: &str = "SELECT DISTINCT ChannelId, GuildId FROM Messages";

// language=sql
const GET_GUILDS_SQL: &str = "SELECT DISTINCT GuildId FROM Messages";

// language=sql
const INSERT_EDIT_SQL: &str = "
INSERT INTO Edits (MessageId, ChannelId, Times, EditContents)
VALUES (?1, ?2, ?3, ?4)";

// Gets the edit id and content of an edit by its message id
// language=sql
const GET_EDIT_ID_CONTENT_BY_ID: &str = "
SELECT EditId, Times, EditContents FROM Edits WHERE MessageId = ?
";

// language=sql
const UPDATE_EDIT_SQL: &str = "
UPDATE Edits SET Times = ?, EditContents = ? WHERE EditId = ?
";
