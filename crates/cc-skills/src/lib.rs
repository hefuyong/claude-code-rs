//! Skill system for Claude Code RS.
//!
//! Skills are reusable prompt templates that can be loaded from
//! the project (`.claude/skills/*.md`), user home (`~/.claude/skills/*.md`),
//! or bundled with the binary.

use cc_error::{CcError, CcResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ── Types ───────────────────────────────────────────────────────────

/// A loaded skill with its prompt content.
#[derive(Debug, Clone)]
pub struct Skill {
    /// The unique name used to invoke the skill.
    pub name: String,
    /// A human-readable description.
    pub description: String,
    /// The prompt content (markdown body after frontmatter).
    pub content: String,
    /// Where this skill was loaded from.
    pub source: SkillSource,
}

/// Origin of a skill definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillSource {
    /// Bundled into the binary.
    Bundled,
    /// From a project-level `.claude/skills/` directory.
    Project(PathBuf),
    /// From the user-level `~/.claude/skills/` directory.
    User(PathBuf),
}

/// YAML frontmatter parsed from a skill file.
#[derive(Debug, Deserialize, Serialize)]
struct SkillFrontmatter {
    name: Option<String>,
    description: Option<String>,
}

// ── Registry ────────────────────────────────────────────────────────

/// Holds all loaded skills, keyed by name.
pub struct SkillRegistry {
    skills: HashMap<String, Skill>,
}

impl SkillRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    /// Load skills from all sources: bundled, project, and user.
    pub async fn load_all(&mut self, project_root: &Path) -> CcResult<()> {
        // 1. Register bundled skills.
        self.register_bundled();

        // 2. Load project-level skills.
        let project_skills_dir = project_root.join(".claude").join("skills");
        self.load_from_dir(&project_skills_dir, |p| SkillSource::Project(p))
            .await;

        // 3. Load user-level skills.
        if let Some(home) = dirs::home_dir() {
            let user_skills_dir = home.join(".claude").join("skills");
            self.load_from_dir(&user_skills_dir, |p| SkillSource::User(p))
                .await;
        }

        tracing::debug!(count = self.skills.len(), "skills loaded");
        Ok(())
    }

    /// Look up a skill by name.
    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    /// List all loaded skills.
    pub fn list(&self) -> Vec<&Skill> {
        let mut list: Vec<&Skill> = self.skills.values().collect();
        list.sort_by_key(|s| &s.name);
        list
    }

    /// Insert a skill into the registry (later sources override earlier).
    fn insert(&mut self, skill: Skill) {
        tracing::debug!(name = %skill.name, "registered skill");
        self.skills.insert(skill.name.clone(), skill);
    }

    /// Register built-in skills.
    fn register_bundled(&mut self) {
        self.insert(Skill {
            name: "simplify".into(),
            description: "Review changed code for reuse, quality, and efficiency".into(),
            content: concat!(
                "Review the recently changed code for:\n",
                "1. Opportunities to reuse existing utilities or abstractions\n",
                "2. Code quality issues (naming, structure, error handling)\n",
                "3. Performance and efficiency improvements\n",
                "\n",
                "Then fix any issues found, keeping changes minimal and focused.",
            )
            .into(),
            source: SkillSource::Bundled,
        });

        self.insert(Skill {
            name: "commit".into(),
            description: "Create a well-structured git commit".into(),
            content: concat!(
                "Analyze all staged and unstaged changes, then:\n",
                "1. Run `git status` and `git diff` to understand the changes\n",
                "2. Draft a concise commit message that focuses on *why*\n",
                "3. Stage relevant files and create the commit\n",
                "\n",
                "Follow conventional commit style when appropriate.",
            )
            .into(),
            source: SkillSource::Bundled,
        });
    }

    /// Scan a directory for `*.md` skill files and load them.
    async fn load_from_dir<F>(&mut self, dir: &Path, make_source: F)
    where
        F: Fn(PathBuf) -> SkillSource,
    {
        let mut read_dir = match tokio::fs::read_dir(dir).await {
            Ok(rd) => rd,
            Err(_) => return, // directory doesn't exist; that's fine
        };

        while let Ok(Some(entry)) = read_dir.next_entry().await {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }

            match self.parse_skill_file(&path, &make_source).await {
                Ok(skill) => self.insert(skill),
                Err(e) => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %e,
                        "failed to parse skill file"
                    );
                }
            }
        }
    }

    /// Parse a single `.md` skill file with optional YAML frontmatter.
    async fn parse_skill_file<F>(
        &self,
        path: &Path,
        make_source: &F,
    ) -> CcResult<Skill>
    where
        F: Fn(PathBuf) -> SkillSource,
    {
        let raw = tokio::fs::read_to_string(path)
            .await
            .map_err(CcError::Io)?;

        let (frontmatter, body) = Self::split_frontmatter(&raw);

        // Derive name from frontmatter or filename.
        let file_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let fm: Option<SkillFrontmatter> = frontmatter.and_then(|fm_str| {
            serde_yaml::from_str(fm_str).ok()
        });

        let name = fm
            .as_ref()
            .and_then(|f| f.name.clone())
            .unwrap_or_else(|| file_name.clone());

        let description = fm
            .as_ref()
            .and_then(|f| f.description.clone())
            .unwrap_or_else(|| format!("Skill from {}", file_name));

        Ok(Skill {
            name,
            description,
            content: body.to_string(),
            source: make_source(path.to_path_buf()),
        })
    }

    /// Split a markdown file into optional YAML frontmatter and body.
    fn split_frontmatter(raw: &str) -> (Option<&str>, &str) {
        let trimmed = raw.trim_start();
        if !trimmed.starts_with("---") {
            return (None, raw);
        }

        // Find the closing `---`.
        let after_open = &trimmed[3..];
        if let Some(close_idx) = after_open.find("\n---") {
            let fm = &after_open[..close_idx];
            let body_start = close_idx + 4; // skip "\n---"
            let body = after_open[body_start..].trim_start_matches('\n');
            (Some(fm.trim()), body)
        } else {
            // No closing delimiter; treat entire content as body.
            (None, raw)
        }
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_frontmatter_with_yaml() {
        let input = "---\nname: test\n---\nBody content here.";
        let (fm, body) = SkillRegistry::split_frontmatter(input);
        assert_eq!(fm, Some("name: test"));
        assert_eq!(body, "Body content here.");
    }

    #[test]
    fn split_frontmatter_without_yaml() {
        let input = "Just plain markdown content.";
        let (fm, body) = SkillRegistry::split_frontmatter(input);
        assert!(fm.is_none());
        assert_eq!(body, input);
    }

    #[test]
    fn bundled_skills_registered() {
        let mut reg = SkillRegistry::new();
        reg.register_bundled();
        assert!(reg.get("simplify").is_some());
        assert!(reg.get("commit").is_some());
    }

    #[test]
    fn list_returns_sorted() {
        let mut reg = SkillRegistry::new();
        reg.register_bundled();
        let list = reg.list();
        let names: Vec<&str> = list.iter().map(|s| s.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[tokio::test]
    async fn load_all_succeeds_with_nonexistent_dirs() {
        let mut reg = SkillRegistry::new();
        let result = reg.load_all(Path::new("/nonexistent/project")).await;
        assert!(result.is_ok());
        // Should still have bundled skills.
        assert!(reg.get("simplify").is_some());
    }
}
