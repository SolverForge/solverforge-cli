use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ListVariableMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distance_meter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intra_distance_meter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_hooks: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub savings_hooks: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub savings_metric_class_fn: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element_owner_fn: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub construction_element_order_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precedence_duration_fn: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precedence_successors_fn: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub solution_trait: Option<String>,
}

impl ListVariableMetadata {
    pub(crate) fn is_empty(&self) -> bool {
        self.entries().iter().all(|(_, value)| value.is_none())
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if let Some(domain) = &self.domain {
            if domain != "cvrp" {
                return Err(format!(
                    "unsupported list variable domain '{domain}'; supported domains: cvrp"
                ));
            }
            for (name, value) in [
                ("distance_meter", &self.distance_meter),
                ("intra_distance_meter", &self.intra_distance_meter),
                ("route_hooks", &self.route_hooks),
                ("savings_hooks", &self.savings_hooks),
                ("savings_metric_class_fn", &self.savings_metric_class_fn),
                ("solution_trait", &self.solution_trait),
            ] {
                if value.is_some() {
                    return Err(format!(
                        "--domain cvrp already provides {name}; remove the explicit --{} flag",
                        name.replace('_', "-")
                    ));
                }
            }
        }

        for (name, value) in self.path_entries() {
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
            ("domain", &self.domain),
            ("distance_meter", &self.distance_meter),
            ("intra_distance_meter", &self.intra_distance_meter),
            ("route_hooks", &self.route_hooks),
            ("savings_hooks", &self.savings_hooks),
            ("savings_metric_class_fn", &self.savings_metric_class_fn),
            ("element_owner_fn", &self.element_owner_fn),
            (
                "construction_element_order_key",
                &self.construction_element_order_key,
            ),
            ("precedence_duration_fn", &self.precedence_duration_fn),
            ("precedence_successors_fn", &self.precedence_successors_fn),
            ("solution_trait", &self.solution_trait),
        ]
    }

    fn path_entries(&self) -> Vec<(&'static str, &Option<String>)> {
        self.entries()
            .into_iter()
            .filter(|(name, _)| *name != "domain")
            .collect()
    }
}
