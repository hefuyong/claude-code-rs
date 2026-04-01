//! Token counting and cost tracking for Claude API usage.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Per-model pricing (USD per million tokens).
#[derive(Debug, Clone)]
pub struct ModelPricing {
    pub input_per_mtok: f64,
    pub output_per_mtok: f64,
    pub cache_write_per_mtok: f64,
    pub cache_read_per_mtok: f64,
}

/// Get pricing for a model. Returns None for unknown models.
pub fn get_pricing(model: &str) -> Option<ModelPricing> {
    // Pricing as of 2025
    match model {
        m if m.contains("opus") => Some(ModelPricing {
            input_per_mtok: 15.0,
            output_per_mtok: 75.0,
            cache_write_per_mtok: 18.75,
            cache_read_per_mtok: 1.50,
        }),
        m if m.contains("sonnet") => Some(ModelPricing {
            input_per_mtok: 3.0,
            output_per_mtok: 15.0,
            cache_write_per_mtok: 3.75,
            cache_read_per_mtok: 0.30,
        }),
        m if m.contains("haiku") => Some(ModelPricing {
            input_per_mtok: 0.80,
            output_per_mtok: 4.0,
            cache_write_per_mtok: 1.0,
            cache_read_per_mtok: 0.08,
        }),
        _ => None,
    }
}

/// Token usage for a single API call.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CallUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
}

/// Tracks cumulative costs across a session.
#[derive(Debug, Default)]
pub struct CostTracker {
    /// Per-model usage accumulator.
    pub model_usage: HashMap<String, CallUsage>,
    /// Total cost in USD.
    pub total_cost_usd: f64,
    /// Total API calls.
    pub total_calls: u64,
}

impl CostTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record usage from a single API call.
    pub fn record(&mut self, model: &str, usage: &CallUsage) {
        let entry = self.model_usage.entry(model.to_string()).or_default();
        entry.input_tokens += usage.input_tokens;
        entry.output_tokens += usage.output_tokens;
        entry.cache_creation_tokens += usage.cache_creation_tokens;
        entry.cache_read_tokens += usage.cache_read_tokens;
        self.total_calls += 1;

        if let Some(pricing) = get_pricing(model) {
            let cost = (usage.input_tokens as f64 * pricing.input_per_mtok
                + usage.output_tokens as f64 * pricing.output_per_mtok
                + usage.cache_creation_tokens as f64 * pricing.cache_write_per_mtok
                + usage.cache_read_tokens as f64 * pricing.cache_read_per_mtok)
                / 1_000_000.0;
            self.total_cost_usd += cost;
        }
    }

    /// Format the total cost for display.
    pub fn format_cost(&self) -> String {
        if self.total_cost_usd < 0.01 {
            format!("${:.4}", self.total_cost_usd)
        } else {
            format!("${:.2}", self.total_cost_usd)
        }
    }

    /// Get total tokens (input + output) across all models.
    pub fn total_tokens(&self) -> u64 {
        self.model_usage
            .values()
            .map(|u| u.input_tokens + u.output_tokens)
            .sum()
    }
}
