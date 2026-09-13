//! LSP wire protocol framing: `Content-Length`-prefixed JSON-RPC messages.
//!
//! ```text
//! Content-Length: 123\r\n
//! \r\n
//! {"jsonrpc":"2.0", ...}
//! ```
//!
//! Kept in its own module (rather than inlined into the manager) so it can
//! be unit-tested in isolation against in-memory buffers — no child
//! process required. This is the piece most worth getting exactly right:
//! every LSP interaction in the app depends on it.

use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Reads one complete LSP message from `reader` and returns its raw JSON
/// body with the header stripped. Returns `Ok(None)` on clean EOF (the
/// process exited without a partial frame in flight).
pub async fn read_message<R: AsyncBufRead + Unpin>(reader: &mut R) -> std::io::Result<Option<String>> {
    let mut content_length: Option<usize> = None;

    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            return Ok(None); // EOF
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break; // blank line: end of headers, body follows
        }
        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            content_length = value.trim().parse::<usize>().ok();
        }
        // Any other header (Content-Type, …) is intentionally ignored —
        // every LSP server in practice only ever sends Content-Length.
    }

    let len = content_length.ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "LSP frame missing Content-Length header")
    })?;

    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).await?;

    String::from_utf8(body)
        .map(Some)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// Writes one complete LSP message, adding the required Content-Length header.
pub async fn write_message<W: AsyncWrite + Unpin>(writer: &mut W, body: &str) -> std::io::Result<()> {
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    writer.write_all(header.as_bytes()).await?;
    writer.write_all(body.as_bytes()).await?;
    writer.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;

    #[tokio::test]
    async fn reads_single_message() {
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#;
        let frame = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
        let mut reader = BufReader::new(frame.as_bytes());
        let msg = read_message(&mut reader).await.unwrap();
        assert_eq!(msg.as_deref(), Some(body));
    }

    #[tokio::test]
    async fn reads_two_messages_back_to_back() {
        let a = r#"{"a":1}"#;
        let b = r#"{"b":2}"#;
        let frame = format!(
            "Content-Length: {}\r\n\r\n{}Content-Length: {}\r\n\r\n{}",
            a.len(),
            a,
            b.len(),
            b
        );
        let mut reader = BufReader::new(frame.as_bytes());
        assert_eq!(read_message(&mut reader).await.unwrap().as_deref(), Some(a));
        assert_eq!(read_message(&mut reader).await.unwrap().as_deref(), Some(b));
    }

    #[tokio::test]
    async fn returns_none_on_clean_eof() {
        let mut reader = BufReader::new(&b""[..]);
        assert_eq!(read_message(&mut reader).await.unwrap(), None);
    }

    #[tokio::test]
    async fn ignores_extra_headers() {
        let body = r#"{"ok":true}"#;
        let frame = format!(
            "Content-Length: {}\r\nContent-Type: application/vscode-jsonrpc; charset=utf-8\r\n\r\n{}",
            body.len(),
            body
        );
        let mut reader = BufReader::new(frame.as_bytes());
        assert_eq!(read_message(&mut reader).await.unwrap().as_deref(), Some(body));
    }

    #[tokio::test]
    async fn errors_on_missing_content_length() {
        let frame = "Content-Type: application/vscode-jsonrpc\r\n\r\n{}";
        let mut reader = BufReader::new(frame.as_bytes());
        assert!(read_message(&mut reader).await.is_err());
    }

    #[tokio::test]
    async fn write_then_read_roundtrip() {
        let mut buf: Vec<u8> = Vec::new();
        write_message(&mut buf, r#"{"roundtrip":true}"#).await.unwrap();
        let mut reader = BufReader::new(buf.as_slice());
        let msg = read_message(&mut reader).await.unwrap();
        assert_eq!(msg.as_deref(), Some(r#"{"roundtrip":true}"#));
    }
}
