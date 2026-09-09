use uuid::Uuid;

/// Current UTC timestamp as RFC-3339, the string form stored in every
/// `*_at` column.
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}
