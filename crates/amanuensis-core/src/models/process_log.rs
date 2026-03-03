use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ProcessLog {
    pub id: i64,
    pub created_at: String,
    pub level: String,
    pub message: String,
}
