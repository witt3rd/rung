//! `todo` — session-local checklist. Kernel state, not a product UI.

use super::Tool;
use serde_json::Value;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub id: String,
    pub content: String,
    pub status: String,
}

#[derive(Clone)]
pub struct Todo {
    items: Arc<Mutex<Vec<Item>>>,
}

impl std::fmt::Debug for Todo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Todo")
    }
}

impl Default for Todo {
    fn default() -> Self {
        Self::new()
    }
}

impl Todo {
    pub fn new() -> Self {
        Self {
            items: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn snapshot(&self) -> Vec<Item> {
        self.items.lock().map(|g| g.clone()).unwrap_or_default()
    }
}

impl Tool for Todo {
    fn name(&self) -> &'static str {
        "todo"
    }
    fn description(&self) -> &'static str {
        "Replace the in-progress todo list. Pass the full list each time. \
         status is pending, in_progress, or completed."
    }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "todos": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": {"type": "string"},
                            "content": {"type": "string"},
                            "status": {"type": "string", "enum": ["pending", "in_progress", "completed"]}
                        },
                        "required": ["id", "content", "status"]
                    }
                }
            },
            "required": ["todos"]
        })
    }
    fn execute(&self, input: &Value) -> Result<String, String> {
        let arr = input["todos"].as_array().ok_or("missing 'todos'")?;
        let mut next = Vec::new();
        for t in arr {
            let id = t["id"].as_str().ok_or("todo missing id")?.to_string();
            let content = t["content"]
                .as_str()
                .ok_or("todo missing content")?
                .to_string();
            let status = t["status"].as_str().unwrap_or("pending").to_string();
            if !matches!(status.as_str(), "pending" | "in_progress" | "completed") {
                return Err(format!("bad todo status: {status}"));
            }
            next.push(Item {
                id,
                content,
                status,
            });
        }
        let mut g = self.items.lock().map_err(|_| "todo lock poisoned")?;
        *g = next;
        Ok(serde_json::to_string_pretty(&*g).unwrap_or_else(|_| "[]".into()))
    }
}

impl serde::Serialize for Item {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut o = s.serialize_struct("Item", 3)?;
        o.serialize_field("id", &self.id)?;
        o.serialize_field("content", &self.content)?;
        o.serialize_field("status", &self.status)?;
        o.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_the_list() {
        let t = Todo::new();
        t.execute(&serde_json::json!({
            "todos": [{"id": "a", "content": "one", "status": "pending"}]
        }))
        .unwrap();
        t.execute(&serde_json::json!({
            "todos": [
                {"id": "a", "content": "one", "status": "completed"},
                {"id": "b", "content": "two", "status": "in_progress"}
            ]
        }))
        .unwrap();
        let snap = t.snapshot();
        assert_eq!(snap.len(), 2);
        assert_eq!(snap[0].status, "completed");
        assert_eq!(snap[1].id, "b");
    }
}
