//! Small shared helpers.

use rand::Rng;

/// Short, sortable-ish id.
pub fn short_id() -> String {
    let mut rng = rand::rng();
    (0..8)
        .map(|_| {
            let i = rng.random_range(0..36);
            char::from_digit(i, 36).unwrap_or('0')
        })
        .collect()
}

pub fn now_iso8601() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}
