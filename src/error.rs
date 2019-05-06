use toml::de::Error as TomlDeserializeError;

#[derive(Debug)]
pub enum StoreError {
    Sqlite(rusqlite::Error),
    JsonError(serde_json::Error),
}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        StoreError::Sqlite(e)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(e: serde_json::Error) -> Self {
        StoreError::JsonError(e)
    }
}

#[derive(Debug)]
pub enum ConfigError {
    NoHome,
    NoParent,

    InvalidGuildFormat,
    InvalidChannelFormat,

    InvalidFormat(TomlDeserializeError),
    Io(std::io::Error),
}

impl From<TomlDeserializeError> for ConfigError {
    fn from(e: TomlDeserializeError) -> Self {
        ConfigError::InvalidFormat(e)
    }
}

impl From<std::io::Error> for ConfigError {
    fn from(e: std::io::Error) -> Self {
        ConfigError::Io(e)
    }
}
