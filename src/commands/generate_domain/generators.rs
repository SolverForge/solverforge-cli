// ─── Code generators ──────────────────────────────────────────────────────────

pub(crate) fn generate_entity(pascal: &str, planning_variable: Option<&str>) -> String {
    let var_field = if let Some(var) = planning_variable {
        format!(
            "    #[planning_variable(allows_unassigned = true)]\n    pub {}: Option<usize>,\n",
            var
        )
    } else {
        String::new()
    };

    let var_init = if let Some(var) = planning_variable {
        format!(", {}: None", var)
    } else {
        String::new()
    };

    format!(
        r#"use serde::{{Deserialize, Serialize}};
use solverforge::prelude::*;

/// TODO — describe this entity.
#[planning_entity]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct {pascal} {{
    #[planning_id]
    pub id: String,
{var_field}}}

impl {pascal} {{
    pub fn new(id: impl Into<String>) -> Self {{
        Self {{ id: id.into(){var_init} }}
    }}
}}
"#
    )
}

pub(crate) fn generate_fact(pascal: &str) -> String {
    format!(
        r#"use serde::{{Deserialize, Serialize}};
use solverforge::prelude::*;

/// TODO — describe this fact.
#[problem_fact]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct {pascal} {{
    pub index: usize,
    pub name: String,
}}

impl {pascal} {{
    pub fn new(index: usize, name: impl Into<String>) -> Self {{
        Self {{ index, name: name.into() }}
    }}
}}
"#
    )
}

pub(crate) fn generate_solution(pascal: &str, score: &str) -> String {
    format!(
        r#"use serde::{{Deserialize, Serialize}};
use solverforge::prelude::*;

#[planning_solution]
#[derive(Serialize, Deserialize)]
pub struct {pascal} {{
    #[planning_score]
    pub score: Option<{score}>,
}}

impl {pascal} {{
    pub fn new() -> Self {{
        Self {{ score: None }}
    }}
}}
"#
    )
}
