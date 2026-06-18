use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, BufRead, Write};

/// A request from the core to a plugin process.
/// Written as a single JSON line to the plugin's stdin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRequest {
    pub id: String,
    pub action: String,
    pub params: Value,
}

/// A response from a plugin process to the core.
/// Read from stdout, one JSON line per response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginResponse {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Write a single PluginRequest as a JSON line to the given writer.
pub fn write_request(writer: &mut impl Write, req: &PluginRequest) -> io::Result<()> {
    let line = serde_json::to_string(req)?;
    writeln!(writer, "{line}")
}

/// Read a single PluginResponse from the given buffered reader.
/// Non-JSON lines are silently skipped.
pub fn read_response(reader: &mut impl BufRead) -> io::Result<PluginResponse> {
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "plugin closed stdin"));
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<PluginResponse>(trimmed) {
            Ok(resp) => return Ok(resp),
            Err(_) => continue, // skip non-JSON lines
        }
    }
}

/// Generate a unique request ID for tracing plugin calls.
pub fn generate_request_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    // Simple short alphanumeric: base62 encode
    base62_encode(n)
}

fn base62_encode(mut n: u64) -> String {
    const CHARS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    if n == 0 {
        return "0".to_string();
    }
    let mut buf = Vec::new();
    while n > 0 {
        buf.push(CHARS[(n % 62) as usize]);
        n /= 62;
    }
    buf.reverse();
    String::from_utf8(buf).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_write_request_roundtrip() {
        let req = PluginRequest {
            id: "t001".to_string(),
            action: "search".to_string(),
            params: json!({"q": "test", "platform": "youtube"}),
        };

        let mut buf: Vec<u8> = Vec::new();
        write_request(&mut buf, &req).unwrap();

        let line = std::str::from_utf8(&buf).unwrap().trim();
        let parsed: PluginRequest = serde_json::from_str(line).unwrap();

        assert_eq!(parsed.id, "t001");
        assert_eq!(parsed.action, "search");
    }

    #[test]
    fn test_read_response() {
        let input = b"{\"id\":\"t001\",\"result\":[\"item1\"]}\n";
        let mut cursor = std::io::Cursor::new(input);
        let resp = read_response(&mut cursor).unwrap();
        assert_eq!(resp.id, "t001");
        assert!(resp.error.is_none());
        assert!(resp.result.is_some());
    }

    #[test]
    fn test_read_response_skips_non_json() {
        let input = b"not json\n{\"id\":\"t002\",\"error\":\"fail\"}\n";
        let mut cursor = std::io::Cursor::new(input);
        let resp = read_response(&mut cursor).unwrap();
        assert_eq!(resp.id, "t002");
        assert_eq!(resp.error, Some("fail".to_string()));
    }

    #[test]
    fn test_base62() {
        assert_eq!(base62_encode(0), "0");
        assert_eq!(base62_encode(61), "z");
        assert_eq!(base62_encode(62), "10");
    }
}
