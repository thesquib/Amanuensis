use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Kill {
    pub id: Option<i64>,
    pub character_id: i64,
    pub creature_name: String,
    pub killed_count: i64,
    pub slaughtered_count: i64,
    pub vanquished_count: i64,
    pub dispatched_count: i64,
    pub assisted_kill_count: i64,
    pub assisted_slaughter_count: i64,
    pub assisted_vanquish_count: i64,
    pub assisted_dispatch_count: i64,
    pub killed_by_count: i64,
    pub date_first: Option<String>,
    pub date_last: Option<String>,
    pub creature_value: i32,
    // Per-type last dates
    pub date_last_killed: Option<String>,
    pub date_last_slaughtered: Option<String>,
    pub date_last_vanquished: Option<String>,
    pub date_last_dispatched: Option<String>,
}

impl Kill {
    pub fn new(character_id: i64, creature_name: String, creature_value: i32) -> Self {
        Self {
            id: None,
            character_id,
            creature_name,
            killed_count: 0,
            slaughtered_count: 0,
            vanquished_count: 0,
            dispatched_count: 0,
            assisted_kill_count: 0,
            assisted_slaughter_count: 0,
            assisted_vanquish_count: 0,
            assisted_dispatch_count: 0,
            killed_by_count: 0,
            date_first: None,
            date_last: None,
            creature_value,
            date_last_killed: None,
            date_last_slaughtered: None,
            date_last_vanquished: None,
            date_last_dispatched: None,
        }
    }

    pub fn total_solo(&self) -> i64 {
        self.killed_count + self.slaughtered_count + self.vanquished_count + self.dispatched_count
    }

    pub fn total_assisted(&self) -> i64 {
        self.assisted_kill_count
            + self.assisted_slaughter_count
            + self.assisted_vanquish_count
            + self.assisted_dispatch_count
    }

    pub fn total_all(&self) -> i64 {
        self.total_solo() + self.total_assisted()
    }
}
