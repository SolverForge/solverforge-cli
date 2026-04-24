use crate::managed_block;

const CUSTOM_ENTITY_TEMPLATE_PATH: &str = ".solverforge/templates/entity.rs.tmpl";
const CUSTOM_SOLUTION_TEMPLATE_PATH: &str = ".solverforge/templates/solution.rs.tmpl";

pub(crate) fn generate_entity(
    pascal: &str,
    planning_variable: Option<&str>,
    extra_fields: &[(String, String)],
) -> Result<String, String> {
    // Custom entity overrides are supported only when they keep the canonical managed blocks.
    let snake = pascal_to_snake(pascal);
    let fields_repr: String = extra_fields
        .iter()
        .map(|(n, t)| format!("pub {}: {}", n, t))
        .collect::<Vec<_>>()
        .join(", ");
    let vars: &[(&str, &str)] = &[
        ("NAME", pascal),
        ("SNAKE_NAME", &snake),
        ("FIELDS", &fields_repr),
    ];
    if let Some(custom) = crate::template::load_custom("entity", vars) {
        return validate_generated_source(
            custom,
            CUSTOM_ENTITY_TEMPLATE_PATH,
            managed_block::ENTITY_REQUIRED_BLOCKS,
        );
    }
    let var_field = if let Some(var) = planning_variable {
        format!(
            "    #[planning_variable(allows_unassigned = true)]\n    pub {}: Option<usize>,\n",
            var
        )
    } else {
        String::new()
    };

    let var_init = if let Some(var) = planning_variable {
        format!("            {}: None,\n", var)
    } else {
        String::new()
    };

    let extra_field_defs: String = extra_fields
        .iter()
        .map(|(n, t)| format!("    pub {}: {},\n", n, t))
        .collect();

    let extra_field_params: String = extra_fields
        .iter()
        .map(|(n, t)| format!(", {}: {}", n, t))
        .collect();

    let extra_field_inits: String = extra_fields
        .iter()
        .map(|(n, _)| format!("            {},\n", n))
        .collect();

    let test_module = generate_entity_test(pascal, planning_variable, extra_fields);

    validate_generated_source(
        format!(
            r#"use serde::{{Deserialize, Serialize}};
use solverforge::prelude::*;

/// TODO — describe this entity.
#[planning_entity]
#[derive(Serialize, Deserialize)]
pub struct {pascal} {{
    #[planning_id]
    pub id: String,
{extra_field_defs}    // @solverforge:begin entity-variables
{var_field}    // @solverforge:end entity-variables
}}

impl {pascal} {{
    pub fn new(id: impl Into<String>{extra_field_params}) -> Self {{
        Self {{
            id: id.into(),
{extra_field_inits}            // @solverforge:begin entity-variable-init
{var_init}            // @solverforge:end entity-variable-init
        }}
    }}
}}
{test_module}"#
        ),
        "built-in entity template",
        managed_block::ENTITY_REQUIRED_BLOCKS,
    )
}

pub(crate) fn generate_fact(pascal: &str, extra_fields: &[(String, String)]) -> String {
    // Check for a custom override in `.solverforge/templates/fact.rs.tmpl`.
    let snake = pascal_to_snake(pascal);
    let fields_repr: String = extra_fields
        .iter()
        .map(|(n, t)| format!("pub {}: {}", n, t))
        .collect::<Vec<_>>()
        .join(", ");
    let vars: &[(&str, &str)] = &[
        ("NAME", pascal),
        ("SNAKE_NAME", &snake),
        ("FIELDS", &fields_repr),
    ];
    if let Some(custom) = crate::template::load_custom("fact", vars) {
        return custom;
    }
    let extra_field_defs: String = extra_fields
        .iter()
        .map(|(n, t)| format!("    pub {}: {},\n", n, t))
        .collect();

    let extra_field_params: String = extra_fields
        .iter()
        .map(|(n, t)| format!(", {}: {}", n, t))
        .collect();

    let extra_field_inits: String = extra_fields
        .iter()
        .map(|(n, _)| format!(", {}", n))
        .collect();

    let test_module = generate_fact_test(pascal, extra_fields);

    format!(
        r#"use serde::{{Deserialize, Serialize}};
use solverforge::prelude::*;

/// TODO — describe this fact.
#[problem_fact]
#[derive(Serialize, Deserialize)]
pub struct {pascal} {{
    #[planning_id]
    pub id: String,
    pub name: String,
{extra_field_defs}}}

impl {pascal} {{
    pub fn new(id: impl Into<String>, name: impl Into<String>{extra_field_params}) -> Self {{
        Self {{ id: id.into(), name: name.into(){extra_field_inits} }}
    }}
}}
{test_module}"#
    )
}

pub(crate) fn generate_solution(pascal: &str, score: &str) -> Result<String, String> {
    // Custom solution overrides are supported only when they keep the canonical managed blocks.
    let snake = pascal_to_snake(pascal);
    let vars: &[(&str, &str)] = &[("NAME", pascal), ("SNAKE_NAME", &snake), ("FIELDS", score)];
    if let Some(custom) = crate::template::load_custom("solution", vars) {
        return validate_generated_source(
            custom,
            CUSTOM_SOLUTION_TEMPLATE_PATH,
            managed_block::SOLUTION_REQUIRED_BLOCKS,
        );
    }
    validate_generated_source(
        format!(
            r#"use serde::{{Deserialize, Serialize}};
use solverforge::prelude::*;

// @solverforge:begin solution-imports
// @solverforge:end solution-imports

#[planning_solution(
    constraints = "crate::constraints::create_constraints",
    solver_toml = "../../solver.toml"
)]
#[derive(Serialize, Deserialize)]
pub struct {pascal} {{
    // @solverforge:begin solution-collections
    // @solverforge:end solution-collections
    #[planning_score]
    pub score: Option<{score}>,
}}

impl {pascal} {{
    #[rustfmt::skip]
    pub fn new(
        // @solverforge:begin solution-constructor-params
        // @solverforge:end solution-constructor-params
    ) -> Self {{
        Self {{
            // @solverforge:begin solution-constructor-init
            // @solverforge:end solution-constructor-init
            score: None,
        }}
    }}
}}
"#
        ),
        "built-in solution template",
        managed_block::SOLUTION_REQUIRED_BLOCKS,
    )
}

fn validate_generated_source(
    src: String,
    source_label: &str,
    required_blocks: &[&str],
) -> Result<String, String> {
    managed_block::require_blocks(&src, required_blocks)
        .map_err(|err| format!("{source_label} is invalid: {err}"))?;
    Ok(src)
}

fn generate_entity_test(
    pascal: &str,
    planning_variable: Option<&str>,
    extra_fields: &[(String, String)],
) -> String {
    let var_assert = if let Some(var) = planning_variable {
        format!("\n        assert!(entity.{}.is_none());", var)
    } else {
        String::new()
    };

    let extra_args = extra_field_test_args(extra_fields);
    let extra_asserts: String = extra_fields
        .iter()
        .map(|(n, _)| format!("\n        let _ = &entity.{};", n))
        .collect();

    format!(
        r#"
#[cfg(test)]
mod tests {{
    use super::*;

    #[test]
    fn test_{snake}_construction() {{
        let entity = {pascal}::new("test-id"{extra_args});
        assert_eq!(entity.id, "test-id");{var_assert}{extra_asserts}
    }}
}}
"#,
        snake = pascal_to_snake(pascal),
        pascal = pascal,
        extra_args = extra_args,
        var_assert = var_assert,
        extra_asserts = extra_asserts,
    )
}

fn generate_fact_test(pascal: &str, extra_fields: &[(String, String)]) -> String {
    let extra_args = extra_field_test_args(extra_fields);
    let extra_asserts: String = extra_fields
        .iter()
        .map(|(n, _)| format!("\n        let _ = &fact.{};", n))
        .collect();

    format!(
        r#"
#[cfg(test)]
mod tests {{
    use super::*;

    #[test]
    fn test_{snake}_construction() {{
        let fact = {pascal}::new("test-id", "test"{extra_args});
        assert_eq!(fact.id, "test-id");
        assert_eq!(fact.name, "test");{extra_asserts}
    }}
}}
"#,
        snake = pascal_to_snake(pascal),
        pascal = pascal,
        extra_args = extra_args,
        extra_asserts = extra_asserts,
    )
}

// Produce default test arguments for extra fields based on type.
fn extra_field_test_args(extra_fields: &[(String, String)]) -> String {
    extra_fields
        .iter()
        .map(|(_, t)| match t.as_str() {
            "String" | "&str" => ", \"test\".to_string()".to_string(),
            "f32" | "f64" => ", 0.0".to_string(),
            "bool" => ", false".to_string(),
            _ if t.starts_with("Option<") => ", None".to_string(),
            _ => ", Default::default()".to_string(),
        })
        .collect()
}

// Simple pascal → snake conversion for test fn names.
fn pascal_to_snake(pascal: &str) -> String {
    let mut result = String::new();
    for (i, c) in pascal.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_ascii_lowercase());
    }
    result
}
