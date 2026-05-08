use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Quality {
    Bad = 0,
    Uncertain = 1,
    Good = 2,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TagSample {
    pub tag: String,
    pub ts: DateTime<Utc>,
    pub value: f64,
    pub quality: Quality,
}
