//! Completeness assertions shared by the generated-app runtime suites.
//!
//! "At least one scalar is assigned" and "assigned list length equals element
//! count" both pass while the plan is wrong: one assignment satisfies the
//! first, and one duplicated element offset by one omitted element satisfies
//! the second. These assertions require every required scalar to be assigned
//! inside its range and every list element to appear exactly once.

use serde_json::Value;

pub fn assert_complete_mixed_solution(solution: &Value) {
    let resources = solution["resources"]
        .as_array()
        .unwrap_or_else(|| panic!("solution should carry resources: {solution:?}"));
    let tasks = solution["tasks"]
        .as_array()
        .unwrap_or_else(|| panic!("solution should carry tasks: {solution:?}"));
    assert!(
        !tasks.is_empty(),
        "seeded solution should carry tasks: {solution:?}"
    );
    for task in tasks {
        let index = task["resource_idx"]
            .as_u64()
            .unwrap_or_else(|| panic!("every required scalar must be assigned, found {task:?}"));
        assert!(
            (index as usize) < resources.len(),
            "scalar assignment {index} is outside the {} resources: {task:?}",
            resources.len()
        );
    }

    let items = solution["items"]
        .as_array()
        .unwrap_or_else(|| panic!("solution should carry items: {solution:?}"));
    let containers = solution["containers"]
        .as_array()
        .unwrap_or_else(|| panic!("solution should carry containers: {solution:?}"));
    let mut placed = Vec::new();
    for container in containers {
        let order = container["item_order"]
            .as_array()
            .unwrap_or_else(|| panic!("item_order should be an array: {container:?}"));
        for element in order {
            placed.push(
                element
                    .as_u64()
                    .unwrap_or_else(|| panic!("item_order holds element indices: {element:?}")),
            );
        }
    }
    placed.sort_unstable();
    let expected: Vec<u64> = (0..items.len() as u64).collect();
    assert_eq!(
        placed, expected,
        "every list element must be placed exactly once: {solution:?}"
    );
}
