use anyhow::Result;
use pitsu_lib::Diff;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pitignore {
    #[serde(rename = "overrides")]
    pub patterns: Vec<(usize, Pattern)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern {
    pub pattern: String,
    starts_with: Option<Arc<str>>,
    ends_with: Option<Arc<str>>,
    pub negated: bool,
}

impl Pitignore {
    pub fn blank() -> Self {
        Self { patterns: Vec::new() }
    }
    pub fn from_repository(root_folder: std::path::PathBuf) -> Result<Self> {
        let pitignore_path = root_folder.join(".pitignore");
        if !pitignore_path.exists() {
            return Ok(Self::blank());
        }

        let contents = std::fs::read_to_string(pitignore_path)?;
        let patterns = contents
            .lines()
            .enumerate()
            .filter_map(|(index, line)| {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    return None;
                }
                let negated = line.starts_with('!');
                let pattern = if negated { &line[1..] } else { line };
                let pattern = pattern.trim();
                if pattern.is_empty() {
                    return None;
                }
                let mut split = pattern.split('*');
                let mut starts_with = None;
                let mut ends_with = None;
                if let Some(first) = split.next() {
                    if !first.is_empty() {
                        starts_with = Some(first.trim().into());
                    }
                }
                if let Some(last) = split.next() {
                    if !last.is_empty() {
                        ends_with = Some(last.trim().into());
                    }
                }
                if split.next().is_some() {
                    // If there are more than one split, it means there are multiple wildcards
                    // which is not supported in this implementation.
                    return None;
                }
                if starts_with.is_none() && ends_with.is_none() {
                    // There is no pattern at all
                    return None;
                }
                Some((
                    index,
                    Pattern {
                        pattern: pattern.into(),
                        starts_with,
                        ends_with,
                        negated,
                    },
                ))
            })
            .collect();

        Ok(Self { patterns })
    }
    pub fn save_to_repository(&self, root_folder: std::path::PathBuf) -> Result<()> {
        let pitignore_path = root_folder.join(".pitignore");
        let mut contents = String::new();
        for (_index, pattern) in &self.patterns {
            if pattern.negated {
                contents.push('!');
            }
            contents.push_str(&pattern.pattern);
            // if let Some(starts_with) = &pattern.starts_with {
            //     contents.push_str(starts_with);
            // }
            // if let Some(ends_with) = &pattern.ends_with {
            //     contents.push_str(ends_with);
            // }
            contents.push('\n');
        }
        std::fs::write(pitignore_path, contents)?;
        Ok(())
    }
    pub fn apply_patterns(&self, diffs: &Arc<[Diff]>) -> Arc<[Diff]> {
        // Iterate over the patterns and filter the diffs, if a diff matches any negated pattern then it WILL NOT be removed, otherwise if it matches any non-negated pattern it will be removed.
        let mut new = Vec::new();
        for diff in Arc::clone(diffs).iter() {
            let mut should_remove = false;
            let mut matches_negated = false;
            for (_index, pattern) in &self.patterns {
                let mut both_match = true;
                if let Some(starts_with) = &pattern.starts_with {
                    if !diff
                        .full_path
                        .trim_start_matches("/")
                        .starts_with(starts_with.trim_start_matches("/"))
                    {
                        both_match = false;
                    }
                }
                if let Some(ends_with) = &pattern.ends_with {
                    if !diff
                        .full_path
                        .trim_start_matches("/")
                        .ends_with(ends_with.trim_start_matches("/"))
                    {
                        both_match = false;
                    }
                }
                if both_match {
                    if pattern.negated {
                        matches_negated = true;
                    } else {
                        should_remove = true;
                    }
                }
            }
            if !matches_negated && should_remove {
                continue;
            } else {
                // If it matches a negated pattern, we keep it, otherwise we remove it.
                new.push(diff.clone());
            }
        }
        new.into()
    }
}
