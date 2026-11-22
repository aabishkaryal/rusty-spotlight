//! Learned preference (FR-12): exponentially decayed counts per item and per
//! (query, item) pair. Recorded only when an action actually runs.

use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Count {
    pub c: f64,
    /// Seconds since the Unix epoch at the last confirmed action.
    pub last: f64,
}

#[derive(Clone, Debug)]
pub struct Usage {
    pub items: HashMap<String, Count>,
    pub pairs: HashMap<(String, String), Count>,
    pub half_life_days: f64,
    pub pair_multiplier: f64,
    pub cap: f64,
}

impl Default for Usage {
    fn default() -> Self {
        Self {
            items: HashMap::new(),
            pairs: HashMap::new(),
            half_life_days: 14.0,
            pair_multiplier: 2.0,
            cap: 8.0,
        }
    }
}

const DAY: f64 = 86_400.0;

impl Usage {
    fn decayed(&self, count: Option<&Count>, now: f64) -> f64 {
        match count {
            Some(k) => {
                let days = ((now - k.last) / DAY).max(0.0);
                k.c * 0.5f64.powf(days / self.half_life_days)
            }
            None => 0.0,
        }
    }

    /// A confirmed action: `query` is the folded text that was in the field.
    pub fn record(&mut self, query: &str, item_id: &str, now: f64) {
        let c = self.decayed(self.items.get(item_id), now) + 1.0;
        self.items.insert(item_id.to_string(), Count { c, last: now });
        // A one-letter query says too little to pin an item to it.
        if query.chars().count() >= 2 {
            let key = (query.to_string(), item_id.to_string());
            let c = self.decayed(self.pairs.get(&key), now) + 1.0;
            self.pairs.insert(key, Count { c, last: now });
        }
    }

    /// Weight used to reorder results inside a tier.
    pub fn weight(&self, query: &str, item_id: &str, now: f64) -> f64 {
        let item = self.decayed(self.items.get(item_id), now);
        let pair = if query.is_empty() {
            0.0
        } else {
            // Avoid allocating the tuple key on every candidate when there is no history.
            if self.pairs.is_empty() {
                0.0
            } else {
                self.decayed(self.pairs.get(&(query.to_string(), item_id.to_string())), now)
            }
        };
        ((1.0 + item).ln() + self.pair_multiplier * (1.0 + pair).ln()).min(self.cap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_halves_after_one_half_life() {
        let mut u = Usage::default();
        u.record("te", "terminal", 0.0);
        let later = 14.0 * DAY;
        let c = u.decayed(u.items.get("terminal"), later);
        assert!((c - 0.5).abs() < 1e-9);
    }

    #[test]
    fn pair_history_outweighs_item_history() {
        let mut u = Usage::default();
        for _ in 0..3 {
            u.record("x", "a", 0.0);
        }
        u.record("te", "b", 0.0);
        // For the query "te", one use under that exact query beats three uses elsewhere.
        assert!(u.weight("te", "b", 0.0) > u.weight("te", "a", 0.0));
    }

    #[test]
    fn weight_is_capped() {
        let mut u = Usage::default();
        for _ in 0..10_000 {
            u.record("q", "a", 0.0);
        }
        assert!(u.weight("q", "a", 0.0) <= u.cap);
    }
}
