/// Retry a fallible operation up to `max_retries` times.
pub fn retry<F, T, E>(max_retries: u32, mut f: F) -> Result<T, E>
where
    F: FnMut() -> Result<T, E>,
{
    let mut last_err = None;
    for _ in 0..max_retries {
        match f() {
            Ok(val) => return Ok(val),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap())
}

/// Truncate a string to `max_len` characters, appending "..." if truncated.
pub fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_owned()
    } else {
        format!("{}...", &s[..max_len])
    }
}

/// Parse a "key=value" string into a tuple.
pub fn parse_kv(s: &str) -> Option<(&str, &str)> {
    s.split_once('=')
}

pub static VERSION: &str = "0.1.0";

pub const MAX_RETRIES: u32 = 3;
pub const BUFFER_SIZE: usize = 8192;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

macro_rules! log_debug {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        eprintln!("[DEBUG] {}", format!($($arg)*));
    };
}
