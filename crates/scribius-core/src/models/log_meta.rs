use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogMeta {
    pub id: Option<i64>,
    pub character_id: i64,
    pub file_path: String,
    pub date_read: String,
}

impl LogMeta {
    pub fn new(character_id: i64, file_path: String, date_read: String) -> Self {
        Self {
            id: None,
            character_id,
            file_path,
            date_read,
        }
    }
}
