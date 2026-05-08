//! Combat log — Feature #15.
//!
//! Bounded ring buffer (capacity 50, kept across combats). Pushed by
//! `execute_combat_actions` after each action resolves; rendered by
//! `paint_combat_log` (Phase 15D).
//!
//! D-Q3=A: bounded ring 50, kept across combats (NOT cleared on OnExit).
//! ~4 KB memory budget.

use bevy::prelude::*;
use std::collections::VecDeque;

/// A single entry in the combat log.
#[derive(Debug, Clone)]
pub struct CombatLogEntry {
    pub message: String,
    pub turn_number: u32,
}

/// Bounded ring-buffer combat log.
///
/// Capacity 50 (D-Q3=A). Kept across combats — `clear()` exists but is
/// not called on `OnExit(Combat)` in v1.
#[derive(Resource, Debug, Clone)]
pub struct CombatLog {
    pub entries: VecDeque<CombatLogEntry>,
    pub capacity: usize,
}

impl Default for CombatLog {
    fn default() -> Self {
        Self {
            entries: VecDeque::with_capacity(50),
            capacity: 50,
        }
    }
}

impl CombatLog {
    /// Append a new entry; pop oldest when over capacity (Pitfall 7).
    pub fn push(&mut self, message: String, turn_number: u32) {
        self.entries.push_back(CombatLogEntry {
            message,
            turn_number,
        });
        while self.entries.len() > self.capacity {
            self.entries.pop_front();
        }
    }

    /// Manual reset (unused in v1 — D-Q3=A keeps log across combats).
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combat_log_caps_at_50() {
        let mut log = CombatLog::default();
        for i in 0..60 {
            log.push(format!("entry {}", i), i);
        }
        assert_eq!(log.entries.len(), 50);
        assert_eq!(log.entries.front().unwrap().message, "entry 10");
        assert_eq!(log.entries.back().unwrap().message, "entry 59");
    }

    #[test]
    fn combat_log_clear_empties() {
        let mut log = CombatLog::default();
        log.push("test".into(), 0);
        log.clear();
        assert!(log.entries.is_empty());
    }
}
