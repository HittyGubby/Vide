use chrono::Local;

/// User-facing date stamp for newly added or backfilled songs.
pub fn now_date() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}
