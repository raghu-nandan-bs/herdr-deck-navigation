use crate::state::FocusTarget;
use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

pub fn socket_path() -> Result<PathBuf> {
    if let Ok(p) = std::env::var("HERDR_SOCKET_PATH") {
        if !p.is_empty() {
            return Ok(PathBuf::from(p));
        }
    }
    let home = std::env::var("HOME").context("HOME not set")?;
    Ok(PathBuf::from(home).join(".config/herdr/herdr.sock"))
}

pub(crate) fn encode_request(id: &str, method: &str, params: Value) -> String {
    // keys serialize alphabetically (id, method, params) — which is exactly the order we want
    format!(
        "{}\n",
        serde_json::to_string(&json!({"id": id, "method": method, "params": params})).unwrap()
    )
}

pub(crate) fn decode_response(line: &str) -> Result<Value> {
    let v: Value = serde_json::from_str(line).context("invalid JSON response")?;
    if let Some(err) = v.get("error") {
        return Err(anyhow!("herdr error: {}", err));
    }
    v.get("result")
        .cloned()
        .ok_or_else(|| anyhow!("response missing `result`"))
}

#[cfg(unix)]
pub fn call(path: &Path, method: &str, params: Value) -> Result<Value> {
    use std::os::unix::net::UnixStream;
    let stream = UnixStream::connect(path)
        .with_context(|| format!("connect {}", path.display()))?;
    let mut writer = stream.try_clone()?;
    writer.write_all(encode_request("deck", method, params).as_bytes())?;
    writer.flush()?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    decode_response(line.trim_end())
}

/// Returns the full JSON response string (envelope) for `model::build_deck`.
pub fn snapshot(path: &Path) -> Result<String> {
    let result = call(path, "session.snapshot", json!({}))?;
    // build_deck expects the `{result:{snapshot:..}}` envelope shape.
    Ok(serde_json::to_string(&json!({ "result": result }))?)
}

pub fn focus(path: &Path, target: &FocusTarget) -> Result<()> {
    let (method, params) = match target {
        FocusTarget::Workspace(id) => ("workspace.focus", json!({ "workspace_id": id })),
        FocusTarget::Tab(id) => ("tab.focus", json!({ "tab_id": id })),
        FocusTarget::Pane(id) => ("pane.focus", json!({ "pane_id": id })),
    };
    call(path, method, params)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn encodes_request_as_newline_delimited_json() {
        let line = encode_request("req1", "session.snapshot", json!({}));
        assert_eq!(line, "{\"id\":\"req1\",\"method\":\"session.snapshot\",\"params\":{}}\n");
    }

    #[test]
    fn extracts_result_or_errors() {
        let ok = decode_response("{\"id\":\"x\",\"result\":{\"type\":\"pong\"}}").unwrap();
        assert_eq!(ok["type"], "pong");
        let err = decode_response("{\"id\":\"x\",\"error\":{\"code\":\"bad\",\"message\":\"nope\"}}");
        assert!(err.is_err());
    }
}
