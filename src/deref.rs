use std::collections::VecDeque;

use log::debug;

#[derive(Debug, Clone)]
pub struct Deref {
    pub map: VecDeque<u64>,
    pub repeated_pattern: bool,
    pub final_assembly: String,
}

impl Deref {
    pub fn new() -> Self {
        Self { map: VecDeque::new(), repeated_pattern: false, final_assembly: String::new() }
    }

    /// Attempts to insert a `u64` value and prevents repeated patterns
    ///
    /// Returns `true` if inserted, `false` otherwise.
    pub fn try_push(&mut self, value: u64) -> bool {
        self.map.push_back(value);

        if self.has_repeating_pattern() {
            self.repeated_pattern = true;
            self.map.pop_back();
            return false;
        }

        true
    }

    fn has_repeating_pattern(&self) -> bool {
        if self.map.len() == 1 {
            return false;
        }
        if self.map.len() == 2 {
            return self.map[0] == self.map[1];
        }

        debug!("map: {:02x?}", self.map);
        for pattern_length in 2..=self.map.len() / 2 {
            for start in 0..(self.map.len() - pattern_length) {
                let first_section: &Vec<u64> =
                    &self.map.range(start..start + pattern_length).copied().collect();
                debug!("1: {first_section:02x?}");

                for second_start in (start + 1)..=(self.map.len() - pattern_length) {
                    let second_section: &Vec<u64> = &self
                        .map
                        .range(second_start..second_start + pattern_length)
                        .copied()
                        .collect();
                    debug!("2: {second_section:02x?}");
                    if first_section == second_section {
                        debug!("found matching");
                        return true;
                    }
                }
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_insert() {
        let mut checker = Deref::new();
        assert!(checker.try_push(1));
    }

    #[test]
    fn test_single_repeating_value_blocked() {
        let mut checker = Deref::new();

        assert!(checker.try_push(1));
        assert!(!checker.try_push(1));
    }

    #[test]
    fn test_no_repeating_single_value() {
        let mut checker = Deref::new();
        checker.try_push(1);
        assert!(!checker.try_push(1));
    }

    #[test]
    fn test_multiple_insertions() {
        let mut checker = Deref::new();
        assert!(checker.try_push(1));
        assert!(checker.try_push(2));
        assert!(checker.try_push(3));
    }

    #[test]
    fn test_repeating_longer_pattern_blocked() {
        let mut checker = Deref::new();
        assert!(checker.try_push(1));
        assert!(checker.try_push(2));
        assert!(checker.try_push(3));
        assert!(checker.try_push(2));
        assert!(!checker.try_push(3));
    }

    // 7fffffffb088: [7fffffffb078, 7fffffffb070, 7fffffffb088, 7fffffffb080, 7fffffffb078, 7fffffffb070]
    #[test]
    fn test_repeated_longer_pattern_blocked_real() {
        let mut checker = Deref::new();
        assert!(checker.try_push(0x7fffffffb078));
        assert!(checker.try_push(0x7fffffffb070));
        assert!(checker.try_push(0x7fffffffb088));
        assert!(checker.try_push(0x7fffffffb080));
        assert!(checker.try_push(0x7fffffffb078));
        assert!(!checker.try_push(0x7fffffffb070));
        // assert_eq!(checker.try_push(0x7fffffffb088), false);
    }

    #[test]
    fn test_non_repeating_pattern_allowed() {
        let mut checker = Deref::new();
        checker.try_push(1);
        checker.try_push(2);
        checker.try_push(3);
        assert!(checker.try_push(4));
    }
}
