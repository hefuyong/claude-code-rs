//! Companion character system for Claude Code RS.
//!
//! Generates a unique companion creature based on a seed value,
//! with randomized species, name, and stats.

use rand::Rng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};

/// A companion character.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Companion {
    /// The species of the companion (e.g. "Fox", "Owl").
    pub species: String,
    /// The companion's name.
    pub name: String,
    /// The companion's stats.
    pub stats: Stats,
}

/// Stat block for a companion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stats {
    /// Physical power.
    pub strength: u8,
    /// Mental acuity.
    pub intelligence: u8,
    /// Social grace.
    pub charisma: u8,
    /// Fortune's favor.
    pub luck: u8,
}

impl Stats {
    /// Total stat points.
    pub fn total(&self) -> u32 {
        self.strength as u32
            + self.intelligence as u32
            + self.charisma as u32
            + self.luck as u32
    }
}

/// Available companion species.
const SPECIES: &[&str] = &[
    "Fox", "Owl", "Cat", "Dragon", "Phoenix", "Wolf", "Raven",
    "Turtle", "Rabbit", "Bear", "Falcon", "Deer", "Otter", "Lynx",
    "Badger", "Crane",
];

/// First name parts.
const NAME_PREFIXES: &[&str] = &[
    "Spark", "Shadow", "Moon", "Star", "Ember", "Frost", "Dawn",
    "Storm", "River", "Cloud", "Blaze", "Sage", "Ash", "Ivy",
    "Crystal", "Echo",
];

/// Second name parts.
const NAME_SUFFIXES: &[&str] = &[
    "whisker", "wing", "claw", "heart", "eye", "paw", "tail",
    "fang", "song", "dance", "leaf", "stone", "light", "shade",
    "breeze", "flame",
];

impl Companion {
    /// Generate a companion from a deterministic seed.
    ///
    /// The same seed will always produce the same companion.
    pub fn generate(seed: u64) -> Self {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);

        let species = SPECIES[rng.gen_range(0..SPECIES.len())].to_string();
        let prefix = NAME_PREFIXES[rng.gen_range(0..NAME_PREFIXES.len())];
        let suffix = NAME_SUFFIXES[rng.gen_range(0..NAME_SUFFIXES.len())];
        let name = format!("{prefix}{suffix}");

        let stats = Stats {
            strength: rng.gen_range(1..=20),
            intelligence: rng.gen_range(1..=20),
            charisma: rng.gen_range(1..=20),
            luck: rng.gen_range(1..=20),
        };

        Self {
            species,
            name,
            stats,
        }
    }

    /// A short display string for the companion.
    pub fn summary(&self) -> String {
        format!(
            "{} the {} (STR:{} INT:{} CHA:{} LCK:{})",
            self.name,
            self.species,
            self.stats.strength,
            self.stats.intelligence,
            self.stats.charisma,
            self.stats.luck,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_generation() {
        let a = Companion::generate(42);
        let b = Companion::generate(42);
        assert_eq!(a.name, b.name);
        assert_eq!(a.species, b.species);
        assert_eq!(a.stats.strength, b.stats.strength);
    }

    #[test]
    fn different_seeds_differ() {
        let a = Companion::generate(1);
        let b = Companion::generate(999);
        // Extremely unlikely to match on all fields.
        let same = a.name == b.name
            && a.species == b.species
            && a.stats.strength == b.stats.strength;
        assert!(!same);
    }

    #[test]
    fn stats_in_range() {
        for seed in 0..100 {
            let c = Companion::generate(seed);
            assert!(c.stats.strength >= 1 && c.stats.strength <= 20);
            assert!(c.stats.intelligence >= 1 && c.stats.intelligence <= 20);
            assert!(c.stats.charisma >= 1 && c.stats.charisma <= 20);
            assert!(c.stats.luck >= 1 && c.stats.luck <= 20);
        }
    }

    #[test]
    fn summary_format() {
        let c = Companion::generate(42);
        let summary = c.summary();
        assert!(summary.contains(&c.name));
        assert!(summary.contains(&c.species));
    }

    #[test]
    fn stats_total() {
        let s = Stats {
            strength: 10,
            intelligence: 15,
            charisma: 8,
            luck: 12,
        };
        assert_eq!(s.total(), 45);
    }
}
