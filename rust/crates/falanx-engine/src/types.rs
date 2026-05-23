#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReviewScore {
    pub readability: u8,
    pub maintainability: u8,
    pub performance: u8,
    pub security: u8,
    pub architecture: u8,
}

impl ReviewScore {
    pub fn composite(&self) -> f32 {
        (self.readability as u32
            + self.maintainability as u32
            + self.performance as u32
            + self.security as u32
            + self.architecture as u32) as f32
            / 5.0
    }

    pub fn delta(&self, other: &Self) -> f32 {
        (self.composite() - other.composite()).abs()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReviewIssue {
    pub location: String,
    pub problem: String,
    pub fix: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RewritePatch {
    pub original: String,
    pub revised: String,
    pub issue_ref: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SessionId(pub String);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum RunState {
    Initializing,
    Scoring,
    Reviewing,
    Rewriting,
    ReScoring,
    PlateauCheck,
    HorizonReset,
    Completed,
    Failed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn score(v: u8) -> ReviewScore {
        ReviewScore {
            readability: v,
            maintainability: v,
            performance: v,
            security: v,
            architecture: v,
        }
    }

    #[test]
    fn composite_is_mean_of_five_fields() {
        let s = ReviewScore {
            readability: 4,
            maintainability: 3,
            performance: 5,
            security: 2,
            architecture: 1,
        };
        assert!((s.composite() - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn composite_uniform_score() {
        assert!((score(3).composite() - 3.0).abs() < f32::EPSILON);
        assert!((score(5).composite() - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn delta_is_absolute_difference_of_composites() {
        let a = score(3);
        let b = score(5);
        assert!((a.delta(&b) - 2.0).abs() < f32::EPSILON);
        assert!((b.delta(&a) - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn delta_same_score_is_zero() {
        let a = score(4);
        assert!(a.delta(&a) < f32::EPSILON);
    }
}
