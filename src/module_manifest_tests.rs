use super::ensure_declarations_only;

#[test]
fn accepts_declarations_reexports_and_the_planning_manifest() {
    ensure_declarations_only(
        r#"//! Docs.

// @solverforge:begin constraint-modules
mod all_assigned;
// @solverforge:end constraint-modules
#[cfg(test)]
mod tests;
mod assemble;

pub use assemble::create_constraints;
use crate::domain::Plan;
"#,
    )
    .expect("declarations only");
    ensure_declarations_only(
        r#"solverforge::planning_model! {
    root = "src/domain";
    mod plan;
    pub use plan::Plan;
}
"#,
    )
    .expect("the planning manifest is a declaration");
}

#[test]
fn rejects_code_in_a_module_root() {
    for (src, needle) in [
        (
            "pub fn create_constraints() {}\n",
            "fn `create_constraints`",
        ),
        (
            "mod assemble { pub fn f() {} }\n",
            "the body of `mod assemble`",
        ),
        ("pub struct Plan;\n", "struct `Plan`"),
        ("const LIMIT: usize = 4;\n", "const `LIMIT`"),
        ("macro_rules! m { () => {} }\n", "the macro invocation"),
        ("impl Foo {}\n", "the impl block"),
    ] {
        let err = ensure_declarations_only(src).expect_err(src);
        assert!(err.contains(needle), "{src}: {err}");
    }
    let err = ensure_declarations_only("#![allow(dead_code)]\nmod a;\n").expect_err("inner attr");
    assert!(err.contains("inner attributes"), "{err}");
}
