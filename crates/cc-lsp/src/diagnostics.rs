//! Diagnostic aggregation.
//!
//! [`DiagnosticRegistry`] collects diagnostics from one or more language
//! servers, keyed by document URI.  It provides query methods for
//! retrieving errors, warnings, and a concise summary that can be
//! displayed to the user or fed into the AI context.

use std::collections::HashMap;

use crate::protocol::{Diagnostic, DiagnosticSeverity};

/// Aggregates diagnostics across all open documents.
pub struct DiagnosticRegistry {
    /// Map from document URI to the list of diagnostics reported for it.
    diagnostics: HashMap<String, Vec<Diagnostic>>,
}

impl DiagnosticRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            diagnostics: HashMap::new(),
        }
    }

    /// Replace all diagnostics for the given URI.
    ///
    /// This is the typical pattern in LSP -- the server publishes the
    /// full set of diagnostics for a document on every change.
    pub fn update(&mut self, uri: &str, diagnostics: Vec<Diagnostic>) {
        if diagnostics.is_empty() {
            self.diagnostics.remove(uri);
        } else {
            self.diagnostics.insert(uri.to_string(), diagnostics);
        }
    }

    /// Get all diagnostics for a URI.  Returns an empty slice when there
    /// are none.
    pub fn get(&self, uri: &str) -> &[Diagnostic] {
        match self.diagnostics.get(uri) {
            Some(v) => v,
            None => &[],
        }
    }

    /// Get only the *error*-level diagnostics for a URI.
    pub fn get_errors(&self, uri: &str) -> Vec<&Diagnostic> {
        self.get(uri)
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Error)
            .collect()
    }

    /// Get only the *warning*-level diagnostics for a URI.
    pub fn get_warnings(&self, uri: &str) -> Vec<&Diagnostic> {
        self.get(uri)
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Warning)
            .collect()
    }

    /// Iterate over all errors across every document.
    pub fn all_errors(&self) -> Vec<(&str, &Diagnostic)> {
        self.diagnostics
            .iter()
            .flat_map(|(uri, diags)| {
                diags
                    .iter()
                    .filter(|d| d.severity == DiagnosticSeverity::Error)
                    .map(move |d| (uri.as_str(), d))
            })
            .collect()
    }

    /// Remove all diagnostics for a URI.
    pub fn clear(&mut self, uri: &str) {
        self.diagnostics.remove(uri);
    }

    /// Produce a quick summary of the entire registry.
    pub fn summary(&self) -> DiagnosticSummary {
        let mut total_errors = 0usize;
        let mut total_warnings = 0usize;
        let mut files_with_issues = 0usize;

        for diags in self.diagnostics.values() {
            let mut file_counted = false;
            for d in diags {
                match d.severity {
                    DiagnosticSeverity::Error => total_errors += 1,
                    DiagnosticSeverity::Warning => total_warnings += 1,
                    _ => {}
                }
                if !file_counted {
                    files_with_issues += 1;
                    file_counted = true;
                }
            }
        }

        DiagnosticSummary {
            total_errors,
            total_warnings,
            files_with_issues,
        }
    }
}

impl Default for DiagnosticRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// A concise summary of the state of the diagnostic registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticSummary {
    pub total_errors: usize,
    pub total_warnings: usize,
    pub files_with_issues: usize,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Position, Range};

    fn make_diag(severity: DiagnosticSeverity, msg: &str) -> Diagnostic {
        Diagnostic {
            range: Range::new(Position::new(0, 0), Position::new(0, 1)),
            severity,
            message: msg.to_string(),
            source: Some("test".to_string()),
        }
    }

    #[test]
    fn test_empty_registry() {
        let reg = DiagnosticRegistry::new();
        assert_eq!(reg.get("file:///x.rs").len(), 0);
        let summary = reg.summary();
        assert_eq!(summary.total_errors, 0);
        assert_eq!(summary.total_warnings, 0);
        assert_eq!(summary.files_with_issues, 0);
    }

    #[test]
    fn test_update_and_get() {
        let mut reg = DiagnosticRegistry::new();
        reg.update(
            "file:///a.rs",
            vec![make_diag(DiagnosticSeverity::Error, "e1")],
        );
        assert_eq!(reg.get("file:///a.rs").len(), 1);
    }

    #[test]
    fn test_get_errors_and_warnings() {
        let mut reg = DiagnosticRegistry::new();
        reg.update(
            "file:///a.rs",
            vec![
                make_diag(DiagnosticSeverity::Error, "err"),
                make_diag(DiagnosticSeverity::Warning, "warn"),
                make_diag(DiagnosticSeverity::Hint, "hint"),
            ],
        );
        assert_eq!(reg.get_errors("file:///a.rs").len(), 1);
        assert_eq!(reg.get_warnings("file:///a.rs").len(), 1);
    }

    #[test]
    fn test_all_errors_across_files() {
        let mut reg = DiagnosticRegistry::new();
        reg.update(
            "file:///a.rs",
            vec![make_diag(DiagnosticSeverity::Error, "e1")],
        );
        reg.update(
            "file:///b.rs",
            vec![make_diag(DiagnosticSeverity::Error, "e2")],
        );
        assert_eq!(reg.all_errors().len(), 2);
    }

    #[test]
    fn test_clear() {
        let mut reg = DiagnosticRegistry::new();
        reg.update(
            "file:///a.rs",
            vec![make_diag(DiagnosticSeverity::Error, "e")],
        );
        reg.clear("file:///a.rs");
        assert!(reg.get("file:///a.rs").is_empty());
    }

    #[test]
    fn test_summary() {
        let mut reg = DiagnosticRegistry::new();
        reg.update(
            "file:///a.rs",
            vec![
                make_diag(DiagnosticSeverity::Error, "e1"),
                make_diag(DiagnosticSeverity::Error, "e2"),
                make_diag(DiagnosticSeverity::Warning, "w1"),
            ],
        );
        reg.update(
            "file:///b.rs",
            vec![make_diag(DiagnosticSeverity::Warning, "w2")],
        );
        let s = reg.summary();
        assert_eq!(s.total_errors, 2);
        assert_eq!(s.total_warnings, 2);
        assert_eq!(s.files_with_issues, 2);
    }

    #[test]
    fn test_update_with_empty_removes() {
        let mut reg = DiagnosticRegistry::new();
        reg.update(
            "file:///a.rs",
            vec![make_diag(DiagnosticSeverity::Error, "e")],
        );
        reg.update("file:///a.rs", vec![]);
        assert!(reg.get("file:///a.rs").is_empty());
        assert_eq!(reg.summary().files_with_issues, 0);
    }
}
