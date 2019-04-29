use rusqlite::{Connection as SqliteConnection, ToSql, NO_PARAMS};
use serenity::model::event::MessageUpdateEvent;
use serenity::model::id::{ChannelId, GuildId, MessageId, UserId};
use serenity::{model::channel::Message, prelude::Mutex};
use std::cell::RefCell;
use std::path::Path;
use std::sync::Arc;

pub struct StatsStore {
    conn: Arc<Mutex<SqliteConnection>>,
    current_user: Mutex<RefCell<Option<UserId>>>,
}

#[derive(serde_derive::Serialize, Debug)]
pub struct Channel {
    pub channel_id: i64,
    pub guild_id: Option<i64>,
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
    pub fn new(path: &Path) -> Result<StatsStore, rusqlite::Error> {
        Ok(StatsStore {
            conn: Arc::new(Mutex::new(StatsStore::setup_connection(path)?)),
            current_user: Mutex::new(RefCell::new(None)),
        })
    }

    fn setup_connection(path: &Path) -> Result<SqliteConnection, rusqlite::Error> {
        let conn = SqliteConnection::open(path).expect("Unable to open database");

        conn.execute(CREATE_MSGS_TABLE_SQL, NO_PARAMS)?;
        conn.execute(CREATE_EDITS_TABLE_SQL, NO_PARAMS)?;
        conn.execute(CREATE_DELETIONS_TABLE_SQL, NO_PARAMS)?;
        Ok(conn)
    }

    pub fn set_current_user(&self, user_id: UserId) {
        *self.current_user.lock().get_mut() = Some(user_id)
    }

    pub fn insert_msg(&self, msg: &Message) {
        let data = &[
            &(msg.id.0 as i64) as &ToSql,
            &msg.timestamp.timestamp(),
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
                let mut times: Vec<i64> = match serde_json::from_str(time) {
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
                    times.push(timestamp.timestamp())
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
                let time = update.edited_timestamp.map(|t| t.timestamp());
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

    pub fn insert_deletion(&self, channel_id: ChannelId, message_id: MessageId) {
        let data = &[
            &(message_id.0 as i64) as &ToSql,
            &(channel_id.0 as i64),
            &chrono::offset::Utc::now().timestamp(),
        ];
        if let Err(e) = self.conn.lock().execute(INSERT_DELETION_SQL, data) {
            eprintln!("Failed to insert message: {}", e);
        }
    }

    pub fn get_message_with_channel_id(
        &self,
        channel_id: ChannelId,
        message_id: MessageId,
    ) -> rusqlite::Result<StoreMessage> {
        let conn = self.conn.lock();
        conn.query_row(
            GET_MSG_BY_CHANNEL_ID_AND_ID,
            &[&(channel_id.0 as i64) as &ToSql, &(message_id.0 as i64)],
            |row| {
                let message_id: i64 = row.get(0)?;
                let channel_id: i64 = row.get(3)?;
                let guild_id: Option<i64> = row.get(4)?;
                let guild_id = guild_id.map(|g| GuildId(g as u64));
                let author_id: i64 = row.get(5)?;
                Ok(StoreMessage {
                    message_id: MessageId(message_id as u64),
                    time: row.get(1)?,
                    content: row.get(2)?,
                    channel_id: ChannelId(channel_id as u64),
                    guild_id,
                    author_id: UserId(author_id as u64),
                })
            },
        )
    }

    pub fn get_msg_count(&self) -> rusqlite::Result<i64> {
        self.conn
            .lock()
            .query_row(GET_MSG_COUNT_SQL, NO_PARAMS, |row| row.get(0))
    }

    pub fn get_user_msg_count(&self) -> rusqlite::Result<i64> {
        // language=sql
        let query = "SELECT COUNT(*)
        FROM Messages
        WHERE AuthorId = ?";

        let id = self.current_user.lock().borrow().unwrap_or(UserId(0)).0 as i64;

        self.conn.lock().query_row(query, &[id], |row| row.get(0))
    }

    pub fn get_user_msgs_per_day(&self) -> rusqlite::Result<Vec<(String, i64, i64)>> {
        // language=sql
        let query = "
        SELECT DATE(Time, 'unixepoch') msg_date, SUM(GuildId IS NOT NULl) msg_count, SUM(GuildId ISNULL)
        From Messages
        WHERE AuthorId = ?
        GROUP BY msg_date
        ORDER BY msg_date ASC";
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(query)?;

        let id = self.current_user.lock().borrow().unwrap_or(UserId(0)).0 as i64;

        stmt.query_map(&[id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .map(|rows| rows.flatten().collect::<Vec<_>>())
    }

    pub fn get_total_msgs_per_day(&self) -> rusqlite::Result<Vec<(String, i64, i64)>> {
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
    }

    pub fn get_edit_count(&self) -> rusqlite::Result<i64> {
        //language=sql
        let query = "
        SELECT SUM(json_array_length(EditContents)) FROM Edits;";

        self.conn
            .lock()
            .query_row(query, NO_PARAMS, |row| row.get(0))
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

    pub fn get_guilds(&self) -> rusqlite::Result<Vec<u64>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(GET_GUILDS_SQL)?;

        stmt.query_map(NO_PARAMS, |row| row.get(0)).map(|rows| {
            let mut out: Vec<Option<i64>> = Vec::new();

            for r in rows {
                if let Ok(i) = r {
                    out.push(i);
                }
            }

            out.iter().flatten().map(|&g| g as u64).collect()
        })
    }
}

// language=sql
const CREATE_MSGS_TABLE_SQL: &str = r#"CREATE TABLE IF NOT EXISTS Messages
(
    EventId    INTEGER PRIMARY KEY,
    MessageId  INTEGER,
    Time       INTEGER,
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
const CREATE_DELETIONS_TABLE_SQL: &str = "
CREATE TABLE IF NOT EXISTS Deletions
(
    DeleteId    INTEGER PRIMARY KEY,
    MessageId   INTEGER,
    ChannelId   INTEGER,
    Time        INTEGER
)
";

// language=sql
const INSERT_DELETION_SQL: &str = "
INSERT into Deletions (MessageId, ChannelId, Time)
VALUES (?1, ?2, ?3)
";

// language=sql
const INSERT_MSG_SQL: &str = r#"INSERT INTO main.Messages
    (MessageId, Time, Content, ChannelId, GuildId, AuthorId)
VALUES (?1, ?2, ?3, ?4, ?5, ?6);"#;

// language=sql
const GET_MSG_BY_CHANNEL_ID_AND_ID: &str = "
SELECT MessageId, Time, Content, ChannelId, GuildId, AuthorId
FROM Messages
WHERE ChannelId = ?1
  AND MessageId = ?2
";

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
