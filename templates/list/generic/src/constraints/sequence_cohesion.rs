use crate::domain::{Container, Plan};
use solverforge::prelude::*;
use solverforge::stream::vec;
use solverforge::IncrementalConstraint;

/// SOFT: Keep each container's sequence locally coherent.
///
/// Penalizes large jumps between adjacent item positions so local search has a
/// visible optimization path after construction.
pub fn constraint() -> impl IncrementalConstraint<Plan, HardSoftScore> {
    ConstraintFactory::<Plan, HardSoftScore>::new()
        .for_each(vec(|p: &Plan| &p.containers))
        .penalize_with(|c: &Container| {
            let gap_penalty: i64 = c
                .items
                .windows(2)
                .map(|pair| {
                    let left = pair[0] as i64;
                    let right = pair[1] as i64;
                    (right - left).abs().saturating_sub(1)
                })
                .sum();
            HardSoftScore::of(0, gap_penalty)
        })
        .named("Sequence cohesion")
}
