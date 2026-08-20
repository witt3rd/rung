//! Opt-in shell. Not part of the default filesystem collection.

use super::Tool;
use serde_json::Value;

#[derive(Debug)]
pub struct Shell;

impl Tool for Shell {
    fn name(&self) -> &'static str {
        "shell"
    }
    fn description(&self) -> &'static str {
        "Execute a shell command via `bash -c`. Returns stdout, stderr, and exit code."
    }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {"type": "string", "description": "Shell command to execute"}
            },
            "required": ["command"]
        })
    }
    fn execute(&self, input: &Value) -> Result<String, String> {
        let command = input["command"].as_str().ok_or("missing 'command'")?;
        if command.trim().is_empty() {
            return Err("shell: empty command".into());
        }
        let destructive = ["rm -rf /", "dd if=", "mkfs.", ":(){ :|:& };:"]
            .iter()
            .any(|pat| command.contains(pat));
        if destructive {
            eprintln!("shell: destructive pattern — '{command}'");
        }
        let output = std::process::Command::new("bash")
            .arg("-c")
            .arg(command)
            .output()
            .map_err(|e| format!("shell: {e}"))?;
        let mut parts = Vec::new();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let code = output.status.code().unwrap_or(-1);
        if !stdout.trim().is_empty() {
            parts.push(stdout.trim().to_string());
        }
        if !stderr.trim().is_empty() {
            parts.push(format!("[stderr]\n{}", stderr.trim()));
        }
        parts.push(format!("[exit: {code}]"));
        Ok(parts.join("\n"))
    }
}
