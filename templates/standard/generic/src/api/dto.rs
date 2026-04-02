use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use solverforge::SolverStatus;

use crate::domain::Plan;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanDto {
    #[serde(flatten)]
    pub fields: Map<String, Value>,
    #[serde(default)]
    pub score: Option<String>,
    #[serde(default)]
    pub solver_status: Option<SolverStatus>,
}

/// Constraint analysis result.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConstraintAnalysisDto {
    pub name: String,
    #[serde(rename = "type")]
    pub constraint_type: String,
    pub weight: String,
    pub score: String,
    pub matches: Vec<ConstraintMatchDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConstraintMatchDto {
    pub score: String,
    pub justification: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeResponse {
    pub score: String,
    pub constraints: Vec<ConstraintAnalysisDto>,
}

impl PlanDto {
    pub fn from_plan(plan: &Plan, status: Option<SolverStatus>) -> Self {
        let mut fields = match serde_json::to_value(plan).expect("failed to serialize plan") {
            Value::Object(map) => map,
            _ => Map::new(),
        };
        let score = fields.remove("score").and_then(|value| {
            if value.is_null() {
                None
            } else if let Some(score) = value.as_str() {
                Some(score.to_string())
            } else {
                Some(value.to_string())
            }
        });

        Self {
            fields,
            score,
            solver_status: status,
        }
    }

    pub fn to_domain(&self) -> Plan {
        let mut fields = self.fields.clone();
        let _ = &self.score;
        fields.insert("score".to_string(), Value::Null);
        serde_json::from_value(Value::Object(fields)).expect("failed to decode plan payload")
    }
}
