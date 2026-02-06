//! Transposition Table implementation.
//!
//! Stores previously evaluated positions to avoid redundant work during search.

use crate::movegen::Move;

/// Type of score stored in a TT entry
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScoreType {
    /// Exact score (PV node)
    Exact,
    /// Lower bound (cut node, score >= beta)
    LowerBound,
    /// Upper bound (all node, score <= alpha)
    UpperBound,
}

/// A single entry in the transposition table
#[derive(Clone, Copy)]
pub struct TTEntry {
    /// Zobrist hash key (full 64-bit for verification)
    pub key: u64,
    /// Depth of the search that produced this entry
    pub depth: u8,
    /// Score (may need adjustment for mate scores)
    pub score: i32,
    /// Type of score (exact, lower bound, upper bound)
    pub score_type: ScoreType,
    /// Best move found (for move ordering)
    pub best_move: Option<Move>,
    /// Age for replacement strategy
    pub age: u8,
}

impl TTEntry {
    /// Create a new empty entry
    pub const fn empty() -> Self {
        TTEntry {
            key: 0,
            depth: 0,
            score: 0,
            score_type: ScoreType::Exact,
            best_move: None,
            age: 0,
        }
    }
}

impl Default for TTEntry {
    fn default() -> Self {
        Self::empty()
    }
}

/// Transposition table
pub struct TranspositionTable {
    /// The hash table entries
    entries: Vec<TTEntry>,
    /// Number of entries (must be power of 2)
    size: usize,
    /// Mask for indexing (size - 1)
    mask: usize,
    /// Current age for replacement
    age: u8,
}

impl TranspositionTable {
    /// Create a new transposition table with the given size in MB
    pub fn new(size_mb: usize) -> Self {
        let entry_size = std::mem::size_of::<TTEntry>();
        let num_entries = (size_mb * 1024 * 1024) / entry_size;
        // Round down to power of 2
        let size = if num_entries > 0 {
            1 << (63 - num_entries.leading_zeros())
        } else {
            1
        };

        TranspositionTable {
            entries: vec![TTEntry::empty(); size],
            size,
            mask: size - 1,
            age: 0,
        }
    }

    /// Clear the transposition table
    pub fn clear(&mut self) {
        for entry in &mut self.entries {
            *entry = TTEntry::empty();
        }
        self.age = 0;
    }

    /// Increment age (call at start of new search)
    pub fn new_search(&mut self) {
        self.age = self.age.wrapping_add(1);
    }

    /// Get the index for a hash key
    #[inline]
    fn index(&self, key: u64) -> usize {
        (key as usize) & self.mask
    }

    /// Probe the transposition table
    /// Returns Some(entry) if found and key matches
    pub fn probe(&self, key: u64) -> Option<&TTEntry> {
        let idx = self.index(key);
        let entry = &self.entries[idx];

        if entry.key == key {
            Some(entry)
        } else {
            None
        }
    }

    /// Store an entry in the transposition table
    /// Uses a replacement strategy that prefers:
    /// 1. Empty entries
    /// 2. Entries from older searches
    /// 3. Entries with lower depth
    pub fn store(
        &mut self,
        key: u64,
        depth: u8,
        score: i32,
        score_type: ScoreType,
        best_move: Option<Move>,
    ) {
        let idx = self.index(key);
        let existing = &self.entries[idx];

        // Replacement strategy
        let should_replace = existing.key == 0  // Empty slot
            || existing.key == key              // Same position (always update)
            || existing.age != self.age         // Old entry from previous search
            || depth >= existing.depth;         // New entry is deeper or equal

        if should_replace {
            self.entries[idx] = TTEntry {
                key,
                depth,
                score,
                score_type,
                best_move,
                age: self.age,
            };
        }
    }

    /// Get the best move from TT if available (for move ordering)
    pub fn get_best_move(&self, key: u64) -> Option<Move> {
        self.probe(key).and_then(|e| e.best_move)
    }

    /// Get fill rate (percentage of entries used)
    pub fn hashfull(&self) -> u32 {
        // Sample first 1000 entries
        let sample_size = self.size.min(1000);
        let used = self.entries[..sample_size]
            .iter()
            .filter(|e| e.key != 0)
            .count();
        ((used * 1000) / sample_size) as u32
    }

    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.size
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }
}

impl Default for TranspositionTable {
    fn default() -> Self {
        Self::new(16) // 16 MB default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tt_creation() {
        let tt = TranspositionTable::new(1); // 1 MB
        assert!(tt.len() > 0);
    }

    #[test]
    fn test_tt_store_and_probe() {
        let mut tt = TranspositionTable::new(1);

        let key = 0x123456789ABCDEF0;
        tt.store(key, 5, 100, ScoreType::Exact, None);

        let entry = tt.probe(key);
        assert!(entry.is_some());

        let entry = entry.unwrap();
        assert_eq!(entry.key, key);
        assert_eq!(entry.depth, 5);
        assert_eq!(entry.score, 100);
        assert_eq!(entry.score_type, ScoreType::Exact);
    }

    #[test]
    fn test_tt_miss() {
        let tt = TranspositionTable::new(1);

        let entry = tt.probe(0x123456789ABCDEF0);
        assert!(entry.is_none());
    }

    #[test]
    fn test_tt_replacement_same_position() {
        let mut tt = TranspositionTable::new(1);

        let key = 0x123456789ABCDEF0;

        // Store initial entry
        tt.store(key, 3, 50, ScoreType::LowerBound, None);

        // Store deeper entry for same position
        tt.store(key, 5, 100, ScoreType::Exact, None);

        let entry = tt.probe(key).unwrap();
        assert_eq!(entry.depth, 5);
        assert_eq!(entry.score, 100);
    }

    #[test]
    fn test_tt_clear() {
        let mut tt = TranspositionTable::new(1);

        let key = 0x123456789ABCDEF0;
        tt.store(key, 5, 100, ScoreType::Exact, None);

        assert!(tt.probe(key).is_some());

        tt.clear();

        assert!(tt.probe(key).is_none());
    }

    #[test]
    fn test_tt_with_best_move() {
        let mut tt = TranspositionTable::new(1);

        let key = 0x123456789ABCDEF0;
        let mv = Move {
            from: 12,
            to: 28,
            promotion: None,
        };

        tt.store(key, 5, 100, ScoreType::Exact, Some(mv));

        let best_move = tt.get_best_move(key);
        assert!(best_move.is_some());
        assert_eq!(best_move.unwrap().from, 12);
        assert_eq!(best_move.unwrap().to, 28);
    }
}
