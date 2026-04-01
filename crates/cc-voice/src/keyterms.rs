//! Voice keyword detection.
//!
//! Detects activation, cancellation, and submission keywords in
//! transcribed text so the voice pipeline can route user intent.

/// Action associated with a detected keyword.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeytermAction {
    /// The user is activating Claude (e.g. "hey claude").
    Activate,
    /// The user wants to cancel the current operation.
    Cancel,
    /// The user wants to submit / confirm the current input.
    Submit,
}

/// A keyword that was found in a transcript.
#[derive(Debug, Clone)]
pub struct DetectedKeyterm {
    /// The keyword that matched.
    pub term: String,
    /// Byte offset of the match within the transcript.
    pub position: usize,
    /// The action this keyword maps to.
    pub action: KeytermAction,
}

/// Keyword detector for voice transcripts.
///
/// Maintains a list of terms and their associated actions, then
/// scans transcripts for the first matching term.
pub struct KeytermDetector {
    terms: Vec<(String, KeytermAction)>,
}

impl Default for KeytermDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl KeytermDetector {
    /// Create a detector pre-loaded with the default keyword set.
    pub fn new() -> Self {
        let terms = vec![
            ("hey claude".to_string(), KeytermAction::Activate),
            ("claude".to_string(), KeytermAction::Activate),
            ("stop".to_string(), KeytermAction::Cancel),
            ("cancel".to_string(), KeytermAction::Cancel),
            ("never mind".to_string(), KeytermAction::Cancel),
            ("submit".to_string(), KeytermAction::Submit),
            ("send".to_string(), KeytermAction::Submit),
            ("go ahead".to_string(), KeytermAction::Submit),
        ];
        Self { terms }
    }

    /// Register an additional keyword with a given action.
    pub fn add_term(&mut self, term: &str, action: KeytermAction) {
        self.terms.push((term.to_lowercase(), action));
    }

    /// Scan a transcript for the first matching keyword.
    ///
    /// Matching is case-insensitive. Longer terms are checked first so that
    /// "hey claude" matches before the shorter "claude".
    pub fn detect(&self, transcript: &str) -> Option<DetectedKeyterm> {
        let lower = transcript.to_lowercase();

        // Sort candidates by length descending so longer phrases match first.
        let mut sorted: Vec<&(String, KeytermAction)> = self.terms.iter().collect();
        sorted.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

        for (term, action) in sorted {
            if let Some(pos) = lower.find(term.as_str()) {
                return Some(DetectedKeyterm {
                    term: term.clone(),
                    position: pos,
                    action: action.clone(),
                });
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_activation() {
        let det = KeytermDetector::new();
        let result = det.detect("Hey Claude, what's the weather?");
        assert!(result.is_some());
        let kw = result.unwrap();
        assert_eq!(kw.action, KeytermAction::Activate);
        assert_eq!(kw.term, "hey claude");
    }

    #[test]
    fn detect_cancel() {
        let det = KeytermDetector::new();
        let result = det.detect("stop doing that");
        assert!(result.is_some());
        assert_eq!(result.unwrap().action, KeytermAction::Cancel);
    }

    #[test]
    fn detect_submit() {
        let det = KeytermDetector::new();
        let result = det.detect("go ahead and run it");
        assert!(result.is_some());
        assert_eq!(result.unwrap().action, KeytermAction::Submit);
    }

    #[test]
    fn no_match_returns_none() {
        let det = KeytermDetector::new();
        assert!(det.detect("the quick brown fox").is_none());
    }

    #[test]
    fn custom_term() {
        let mut det = KeytermDetector::new();
        det.add_term("execute", KeytermAction::Submit);
        let result = det.detect("please execute the plan");
        assert!(result.is_some());
        assert_eq!(result.unwrap().action, KeytermAction::Submit);
    }

    #[test]
    fn longer_phrase_preferred() {
        let det = KeytermDetector::new();
        // "hey claude" should match before "claude" alone.
        let result = det.detect("hey claude").unwrap();
        assert_eq!(result.term, "hey claude");
    }

    #[test]
    fn case_insensitive() {
        let det = KeytermDetector::new();
        let result = det.detect("CANCEL everything");
        assert!(result.is_some());
        assert_eq!(result.unwrap().action, KeytermAction::Cancel);
    }
}
