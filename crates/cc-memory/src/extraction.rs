//! Automatic memory extraction from conversation transcripts.
//!
//! Analyses conversation messages to identify facts worth remembering,
//! categorises them, and persists them to the memory directory.

use std::path::PathBuf;

use cc_api::types::{ApiContent, ApiMessage, ContentBlock};
use cc_error::{CcError, CcResult};

/// Category that describes the origin / purpose of an extracted memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryCategory {
    /// Preferences and facts about the user.
    User,
    /// Feedback the user gave about Claude's behaviour.
    Feedback,
    /// Project-specific conventions, architecture decisions, etc.
    Project,
    /// Reference material (links, documentation pointers).
    Reference,
}

impl MemoryCategory {
    /// Return a short slug used in filenames.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Feedback => "feedback",
            Self::Project => "project",
            Self::Reference => "reference",
        }
    }
}

/// A single memory extracted from a conversation.
#[derive(Debug, Clone)]
pub struct ExtractedMemory {
    /// What kind of memory this is.
    pub category: MemoryCategory,
    /// The textual content to persist.
    pub content: String,
    /// The conversation turn number the memory was drawn from.
    pub source_turn: usize,
}

/// Extracts and persists memories from conversation messages.
pub struct MemoryExtractor {
    project_root: PathBuf,
    memory_dir: PathBuf,
}

impl MemoryExtractor {
    /// Create a new extractor.
    ///
    /// Memories will be saved under `<project_root>/.claude/memory/`.
    pub fn new(project_root: PathBuf) -> Self {
        let memory_dir = project_root.join(".claude").join("memory");
        Self {
            project_root,
            memory_dir,
        }
    }

    /// Analyse conversation messages and return any memories worth saving.
    ///
    /// The heuristic scans user messages for:
    /// - Explicit preferences ("I prefer ...", "always use ...")
    /// - Feedback ("don't do that", "good job on ...")
    /// - Project conventions ("we use ...", "our convention is ...")
    /// - Reference links (URLs, doc pointers)
    pub async fn extract_from_messages(
        &self,
        messages: &[ApiMessage],
    ) -> Vec<ExtractedMemory> {
        let mut memories = Vec::new();

        for (turn, msg) in messages.iter().enumerate() {
            if msg.role != "user" {
                continue;
            }

            let text = Self::message_text(msg);
            if text.is_empty() {
                continue;
            }

            // User preference patterns
            if Self::matches_preference(&text) {
                memories.push(ExtractedMemory {
                    category: MemoryCategory::User,
                    content: text.clone(),
                    source_turn: turn,
                });
                continue;
            }

            // Feedback patterns
            if Self::matches_feedback(&text) {
                memories.push(ExtractedMemory {
                    category: MemoryCategory::Feedback,
                    content: text.clone(),
                    source_turn: turn,
                });
                continue;
            }

            // Project convention patterns
            if Self::matches_project(&text) {
                memories.push(ExtractedMemory {
                    category: MemoryCategory::Project,
                    content: text.clone(),
                    source_turn: turn,
                });
                continue;
            }

            // Reference patterns (URLs)
            if Self::matches_reference(&text) {
                memories.push(ExtractedMemory {
                    category: MemoryCategory::Reference,
                    content: text.clone(),
                    source_turn: turn,
                });
            }
        }

        tracing::debug!(
            count = memories.len(),
            project = %self.project_root.display(),
            "extracted memories from conversation"
        );
        memories
    }

    /// Save a single extracted memory to disk.
    ///
    /// Files are named `<category>-<uuid>.md` inside the memory directory.
    pub async fn save_memory(&self, memory: &ExtractedMemory) -> CcResult<PathBuf> {
        tokio::fs::create_dir_all(&self.memory_dir)
            .await
            .map_err(|e| CcError::Io(e))?;

        let id = uuid::Uuid::new_v4();
        let filename = format!("{}-{}.md", memory.category.slug(), id);
        let path = self.memory_dir.join(&filename);

        let content = format!(
            "<!-- auto-extracted | turn {} | category: {} -->\n{}",
            memory.source_turn,
            memory.category.slug(),
            memory.content,
        );

        tokio::fs::write(&path, &content)
            .await
            .map_err(|e| CcError::Io(e))?;

        tracing::debug!(path = %path.display(), "saved extracted memory");
        Ok(path)
    }

    /// Build a prompt that could be sent to an LLM for more advanced extraction.
    pub fn build_extraction_prompt(messages: &[ApiMessage]) -> String {
        let mut prompt = String::from(
            "Analyze the following conversation and extract any facts that should be \
             remembered for future sessions. Categorize each as: user preference, \
             feedback, project convention, or reference.\n\n",
        );

        for (i, msg) in messages.iter().enumerate() {
            let text = Self::message_text(msg);
            if !text.is_empty() {
                prompt.push_str(&format!("[Turn {} - {}]: {}\n", i, msg.role, text));
            }
        }

        prompt.push_str(
            "\nFor each memory, output a JSON object with fields: \
             category (user|feedback|project|reference), content (string).\n",
        );
        prompt
    }

    // --- pattern-matching helpers ---

    fn message_text(msg: &ApiMessage) -> String {
        match &msg.content {
            ApiContent::Text(t) => t.clone(),
            ApiContent::Blocks(blocks) => blocks
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(" "),
        }
    }

    fn matches_preference(text: &str) -> bool {
        let lower = text.to_lowercase();
        let patterns = [
            "i prefer",
            "always use",
            "never use",
            "i like to",
            "my preference is",
            "i want you to",
            "remember that i",
            "from now on",
        ];
        patterns.iter().any(|p| lower.contains(p))
    }

    fn matches_feedback(text: &str) -> bool {
        let lower = text.to_lowercase();
        let patterns = [
            "don't do that",
            "stop doing",
            "good job",
            "that was wrong",
            "that's not right",
            "please don't",
            "i didn't ask for",
            "too verbose",
            "too short",
        ];
        patterns.iter().any(|p| lower.contains(p))
    }

    fn matches_project(text: &str) -> bool {
        let lower = text.to_lowercase();
        let patterns = [
            "we use",
            "our convention",
            "our codebase",
            "in this project",
            "the project uses",
            "our stack is",
            "we follow",
        ];
        patterns.iter().any(|p| lower.contains(p))
    }

    fn matches_reference(text: &str) -> bool {
        text.contains("http://") || text.contains("https://")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_api::types::{ApiContent, ApiMessage};

    fn user_msg(text: &str) -> ApiMessage {
        ApiMessage {
            role: "user".to_string(),
            content: ApiContent::Text(text.to_string()),
        }
    }

    fn assistant_msg(text: &str) -> ApiMessage {
        ApiMessage {
            role: "assistant".to_string(),
            content: ApiContent::Text(text.to_string()),
        }
    }

    #[tokio::test]
    async fn extract_user_preference() {
        let extractor = MemoryExtractor::new(PathBuf::from("/tmp/test"));
        let messages = vec![
            user_msg("I prefer tabs over spaces"),
            assistant_msg("Got it, I'll use tabs."),
        ];
        let memories = extractor.extract_from_messages(&messages).await;
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].category, MemoryCategory::User);
    }

    #[tokio::test]
    async fn extract_feedback() {
        let extractor = MemoryExtractor::new(PathBuf::from("/tmp/test"));
        let messages = vec![user_msg("That was wrong, the function should return a Result")];
        let memories = extractor.extract_from_messages(&messages).await;
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].category, MemoryCategory::Feedback);
    }

    #[tokio::test]
    async fn extract_project_convention() {
        let extractor = MemoryExtractor::new(PathBuf::from("/tmp/test"));
        let messages = vec![user_msg("We use PostgreSQL in this project")];
        let memories = extractor.extract_from_messages(&messages).await;
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].category, MemoryCategory::Project);
    }

    #[tokio::test]
    async fn extract_reference() {
        let extractor = MemoryExtractor::new(PathBuf::from("/tmp/test"));
        let messages = vec![user_msg("Check https://docs.rs/tokio for details")];
        let memories = extractor.extract_from_messages(&messages).await;
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].category, MemoryCategory::Reference);
    }

    #[tokio::test]
    async fn ignores_assistant_messages() {
        let extractor = MemoryExtractor::new(PathBuf::from("/tmp/test"));
        let messages = vec![
            assistant_msg("I prefer to use Rust for this."),
        ];
        let memories = extractor.extract_from_messages(&messages).await;
        assert!(memories.is_empty());
    }

    #[test]
    fn build_prompt_includes_messages() {
        let messages = vec![
            user_msg("hello"),
            assistant_msg("hi there"),
        ];
        let prompt = MemoryExtractor::build_extraction_prompt(&messages);
        assert!(prompt.contains("[Turn 0 - user]: hello"));
        assert!(prompt.contains("[Turn 1 - assistant]: hi there"));
        assert!(prompt.contains("category"));
    }

    #[test]
    fn category_slug() {
        assert_eq!(MemoryCategory::User.slug(), "user");
        assert_eq!(MemoryCategory::Feedback.slug(), "feedback");
        assert_eq!(MemoryCategory::Project.slug(), "project");
        assert_eq!(MemoryCategory::Reference.slug(), "reference");
    }
}
