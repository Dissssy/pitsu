use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pitignore {
    #[serde(rename = "overrides")]
    pub patterns: Vec<Pattern>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern {
    pub pattern: Arc<str>,

    pub negated: bool,
}

impl Pitignore {
    pub fn blank() -> Self {
        Self {
            patterns: Vec::new(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }
    pub fn from_repository(root_folder: std::path::PathBuf) -> Result<Self> {
        let pitignore_path = root_folder.join(".pitignore");
        if !pitignore_path.exists() {
            return Ok(Self::blank());
        }

        let contents = std::fs::read_to_string(pitignore_path)?;
        let patterns = contents
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    return None;
                }
                let negated = line.starts_with('!');
                let pattern = if negated { &line[1..] } else { line };
                Some(Pattern {
                    pattern: pattern.into(),
                    negated,
                })
            })
            .collect();

        Ok(Self { patterns })
    }
}
