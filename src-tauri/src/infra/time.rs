use chrono::Local;
use std::path::Path;
use std::time::SystemTime;

/// Timestamp format for `added_time`: precise to the second, and sortable as
/// a plain string (queries compare it lexicographically).
pub const TIMESTAMP_FORMAT: &str = "%Y/%m/%d %H:%M:%S";

/// Same idea for the tag-usage history: millisecond precision so the most
/// recently added tag really comes first, even within the same second.
pub const PRECISE_FORMAT: &str = "%Y/%m/%d %H:%M:%S%.3f";

/// Timestamp for a newly added song.
pub fn now_timestamp() -> String {
    Local::now().format(TIMESTAMP_FORMAT).to_string()
}

/// Millisecond-precision timestamp (tag history ordering).
pub fn now_precise() -> String {
    Local::now().format(PRECISE_FORMAT).to_string()
}

/// "Added" time of a local file: its modification time, in local time.
///
/// The library treats the file's own timestamp as the meaningful "when did
/// this appear" value; import time is only used when the file cannot be
/// inspected.
pub fn timestamp_from_system_time(time: SystemTime) -> String {
    chrono::DateTime::<Local>::from(time)
        .format(TIMESTAMP_FORMAT)
        .to_string()
}

pub fn timestamp_from_path(path: &Path) -> Option<String> {
    std::fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .map(timestamp_from_system_time)
}
