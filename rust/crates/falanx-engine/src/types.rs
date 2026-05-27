#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CategoryResult {
    pub name: String,
    pub score: u8,
    pub reasoning: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScoringResult {
    pub pipeline_name: String,
    pub categories: Vec<CategoryResult>,
    pub composite_score: f32,
    pub synthesis: String,
}

impl ScoringResult {
    pub fn composite(&self) -> f32 {
        self.composite_score
    }

    pub fn delta(&self, other: &Self) -> f32 {
        (self.composite_score - other.composite_score).abs()
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

#[test]
    fn category_result_serialises_to_json() {
        let r = CategoryResult {
            name: "readability".into(),
            score: 4,
            reasoning: "clear names".into(),
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"name\":\"readability\""));
        assert!(json.contains("\"score\":4"));
    }

    #[test]
    fn scoring_result_composite_returns_composite_score() {
        let r = ScoringResult {
            pipeline_name: "default".into(),
            categories: vec![],
            composite_score: 3.5,
            synthesis: "ok".into(),
        };
        assert!((r.composite() - 3.5).abs() < f32::EPSILON);
    }

    #[test]
    fn scoring_result_delta_is_absolute_difference() {
        let a = ScoringResult {
            pipeline_name: "default".into(),
            categories: vec![],
            composite_score: 3.0,
            synthesis: "".into(),
        };
        let b = ScoringResult {
            pipeline_name: "default".into(),
            categories: vec![],
            composite_score: 4.5,
            synthesis: "".into(),
        };
        assert!((a.delta(&b) - 1.5).abs() < f32::EPSILON);
        assert!((b.delta(&a) - 1.5).abs() < f32::EPSILON);
    }
}
