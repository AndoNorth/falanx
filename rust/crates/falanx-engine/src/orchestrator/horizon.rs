pub struct HorizonState {
    pub reset_count: u32,
    pub max_resets: u32,
}

impl HorizonState {
    pub fn new() -> Self {
        Self { reset_count: 0, max_resets: 2 }
    }

    pub fn should_reset(&self, plateau: bool) -> bool {
        plateau && !self.exhausted()
    }

    pub fn record_reset(&mut self) {
        self.reset_count += 1;
    }

    pub fn exhausted(&self) -> bool {
        self.reset_count >= self.max_resets
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_at_zero_with_max_two() {
        let h = HorizonState::new();
        assert_eq!(h.reset_count, 0);
        assert_eq!(h.max_resets, 2);
        assert!(!h.exhausted());
    }

    #[test]
    fn should_reset_when_plateau_and_not_exhausted() {
        let h = HorizonState::new();
        assert!(h.should_reset(true));
        assert!(!h.should_reset(false));
    }

    #[test]
    fn exhausted_after_max_resets() {
        let mut h = HorizonState::new();
        h.record_reset();
        assert!(!h.exhausted());
        h.record_reset();
        assert!(h.exhausted());
        assert!(!h.should_reset(true));
    }
}
