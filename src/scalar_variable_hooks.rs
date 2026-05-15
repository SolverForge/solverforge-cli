use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ScalarVariableHooks {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_values: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nearby_value_candidates: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nearby_entity_candidates: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nearby_value_distance_meter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nearby_entity_distance_meter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub construction_entity_order_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub construction_value_order_key: Option<String>,
}

impl ScalarVariableHooks {
    pub(crate) fn is_empty(&self) -> bool {
        self.entries().iter().all(|(_, value)| value.is_none())
    }

    pub(crate) fn validate_paths(&self) -> Result<(), String> {
        for (name, value) in self.entries() {
            let Some(path) = value else {
                continue;
            };
            if path.trim().is_empty() {
                return Err(format!("{name} must not be empty"));
            }
            syn::parse_str::<syn::Path>(path)
                .map_err(|_| format!("{name} must be a valid Rust path"))?;
        }
        Ok(())
    }

    pub(crate) fn entries(&self) -> Vec<(&'static str, &Option<String>)> {
        vec![
            ("candidate_values", &self.candidate_values),
            ("nearby_value_candidates", &self.nearby_value_candidates),
            ("nearby_entity_candidates", &self.nearby_entity_candidates),
            (
                "nearby_value_distance_meter",
                &self.nearby_value_distance_meter,
            ),
            (
                "nearby_entity_distance_meter",
                &self.nearby_entity_distance_meter,
            ),
            (
                "construction_entity_order_key",
                &self.construction_entity_order_key,
            ),
            (
                "construction_value_order_key",
                &self.construction_value_order_key,
            ),
        ]
    }
}
