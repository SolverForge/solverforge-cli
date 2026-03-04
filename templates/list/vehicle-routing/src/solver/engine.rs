//! Local search engine — wires the move selectors, acceptor, and phase.

use solverforge_core::domain::{EntityDescriptor, SolutionDescriptor, TypedEntityExtractor};
use solverforge_scoring::{ScoreDirector, TypedScoreDirector};
use solverforge_solver::heuristic::r#move::ListMoveImpl;
use solverforge_solver::heuristic::selector::decorator::{
    SelectedCountLimitMoveSelector, UnionMoveSelector,
};
use solverforge_solver::heuristic::selector::k_opt::{
    KOptConfig, ListPositionDistanceMeter, NearbyKOptMoveSelector,
};
use solverforge_solver::heuristic::selector::nearby_list_change::{
    CrossEntityDistanceMeter, ListMoveNearbyListChangeSelector, NearbyListChangeMoveSelector,
};
use solverforge_solver::heuristic::selector::nearby_list_swap::{
    ListMoveNearbyListSwapSelector, NearbyListSwapMoveSelector,
};
use solverforge_solver::heuristic::selector::typed_move_selector::MoveSelector as MoveSelectorTrait;
use solverforge_solver::heuristic::selector::{
    FromSolutionEntitySelector, ListMoveListReverseSelector, ListMoveListRuinSelector,
    ListMoveSubListChangeSelector, ListReverseMoveSelector, ListRuinMoveSelector,
    SubListChangeMoveSelector,
};
use solverforge_solver::phase::localsearch::{
    AcceptedCountForager, LateAcceptanceAcceptor, LocalSearchPhase,
};
use solverforge_solver::phase::Phase;
use solverforge_solver::scope::SolverScope;
use std::any::TypeId;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::time::Duration;

use crate::constraints::create_constraints;
use crate::domain::{ProblemData, Vehicle, VrpPlan};
use crate::solver::moves::{
    list_get, list_insert, list_len, list_remove, list_reverse, list_set,
    ruin_insert, ruin_remove, sublist_insert, sublist_remove,
};

// ============================================================================
// Distance meters
// ============================================================================

/// Inter-route distance: distance between two visits in different (or same) vehicles.
#[derive(Debug)]
struct VrpDistanceMeter;

impl CrossEntityDistanceMeter<VrpPlan> for VrpDistanceMeter {
    fn distance(
        &self,
        solution: &VrpPlan,
        src_entity: usize,
        src_pos: usize,
        dst_entity: usize,
        dst_pos: usize,
    ) -> f64 {
        let src = &solution.vehicles[src_entity];
        let dst = &solution.vehicles[dst_entity];
        if src_pos >= src.visits.len() || dst_pos >= dst.visits.len() {
            return f64::INFINITY;
        }
        let data = unsafe { &*(src.data as *const ProblemData) };
        data.distance_matrix[src.visits[src_pos]][dst.visits[dst_pos]] as f64
    }
}

/// Intra-route distance: distance between two positions within the same vehicle's route.
#[derive(Debug)]
struct VrpIntraDistanceMeter;

impl ListPositionDistanceMeter<VrpPlan> for VrpIntraDistanceMeter {
    fn distance(&self, solution: &VrpPlan, entity_idx: usize, pos_a: usize, pos_b: usize) -> f64 {
        let v = &solution.vehicles[entity_idx];
        if pos_a >= v.visits.len() || pos_b >= v.visits.len() {
            return f64::INFINITY;
        }
        let data = unsafe { &*(v.data as *const ProblemData) };
        data.distance_matrix[v.visits[pos_a]][v.visits[pos_b]] as f64
    }
}

// ============================================================================
// KOpt wrapper — adapts NearbyKOptMoveSelector to yield ListMoveImpl::KOpt
// ============================================================================

struct ListMoveNearbyKOptSelector<S, V, D: ListPositionDistanceMeter<S>, ES> {
    inner: NearbyKOptMoveSelector<S, V, D, ES>,
    _phantom: PhantomData<fn() -> V>,
}

impl<S, V: Debug, D: ListPositionDistanceMeter<S> + Debug, ES: Debug> Debug
    for ListMoveNearbyKOptSelector<S, V, D, ES>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ListMoveNearbyKOptSelector").field("inner", &self.inner).finish()
    }
}

impl<S, V, D: ListPositionDistanceMeter<S>, ES> ListMoveNearbyKOptSelector<S, V, D, ES> {
    fn new(inner: NearbyKOptMoveSelector<S, V, D, ES>) -> Self {
        Self { inner, _phantom: PhantomData }
    }
}

impl<S, V, D, ES> MoveSelectorTrait<S, ListMoveImpl<S, V>>
    for ListMoveNearbyKOptSelector<S, V, D, ES>
where
    S: solverforge_core::domain::PlanningSolution,
    V: Clone + PartialEq + Send + Sync + Debug + 'static,
    D: ListPositionDistanceMeter<S> + 'static,
    ES: solverforge_solver::heuristic::selector::entity::EntitySelector<S>,
{
    fn iter_moves<'a, SD: ScoreDirector<S>>(
        &'a self,
        score_director: &'a SD,
    ) -> impl Iterator<Item = ListMoveImpl<S, V>> + 'a {
        self.inner.iter_moves(score_director).map(ListMoveImpl::KOpt)
    }

    fn size<SD: ScoreDirector<S>>(&self, score_director: &SD) -> usize {
        self.inner.size(score_director)
    }
}

// ============================================================================
// Descriptor
// ============================================================================

fn entity_counter(plan: &VrpPlan, _descriptor_index: usize) -> usize {
    plan.vehicles.len()
}

fn entity_count(plan: &VrpPlan) -> usize {
    plan.vehicles.len()
}

fn build_descriptor() -> SolutionDescriptor {
    let extractor = Box::new(TypedEntityExtractor::new(
        "Vehicle",
        "vehicles",
        |p: &VrpPlan| &p.vehicles,
        |p: &mut VrpPlan| &mut p.vehicles,
    ));
    let entity_desc =
        EntityDescriptor::new("Vehicle", TypeId::of::<Vehicle>(), "vehicles")
            .with_extractor(extractor);
    SolutionDescriptor::new("VrpPlan", TypeId::of::<VrpPlan>()).with_entity(entity_desc)
}

// ============================================================================
// Public solve function
// ============================================================================

pub fn solve(mut plan: VrpPlan, time_limit_secs: u64) -> VrpPlan {
    crate::solver::construction::run(&mut plan);

    if plan.vehicles.is_empty() {
        return plan;
    }

    let descriptor = build_descriptor();
    let mut director =
        TypedScoreDirector::with_descriptor(plan, create_constraints(), descriptor, entity_counter);

    // 1. NearbyListChange — relocate a visit to one of the 20 nearest destinations
    let list_change = ListMoveNearbyListChangeSelector::new(
        NearbyListChangeMoveSelector::<VrpPlan, usize, _, _>::new(
            FromSolutionEntitySelector::new(0),
            VrpDistanceMeter,
            20,
            list_len,
            list_remove,
            list_insert,
            "visits",
            0,
        ),
    );

    // 2. NearbyListSwap — swap a visit with one of its 20 nearest neighbours
    let list_swap = ListMoveNearbyListSwapSelector::new(
        NearbyListSwapMoveSelector::<VrpPlan, usize, _, _>::new(
            FromSolutionEntitySelector::new(0),
            VrpDistanceMeter,
            20,
            list_len,
            list_get,
            list_set,
            "visits",
            0,
        ),
    );

    // 3. ListReverse — intra-route 2-opt (reverse a segment within one route)
    let list_reverse_sel = ListMoveListReverseSelector::new(
        ListReverseMoveSelector::<VrpPlan, usize, _>::new(
            FromSolutionEntitySelector::new(0),
            list_len,
            list_reverse,
            "visits",
            0,
        ),
    );

    // 4. SubListChange — Or-opt: relocate segments of size 1..=3, capped at 500 per step
    let sublist_change = SelectedCountLimitMoveSelector::new(
        ListMoveSubListChangeSelector::new(SubListChangeMoveSelector::<VrpPlan, usize, _>::new(
            FromSolutionEntitySelector::new(0),
            1,
            3,
            list_len,
            sublist_remove,
            sublist_insert,
            "visits",
            0,
        )),
        500,
    );

    // 5. NearbyKOpt (3-opt intra-route)
    let kopt_sel = ListMoveNearbyKOptSelector::new(
        NearbyKOptMoveSelector::<VrpPlan, usize, _, _>::new(
            FromSolutionEntitySelector::new(0),
            VrpIntraDistanceMeter,
            10,
            KOptConfig::new(3),
            list_len,
            sublist_remove,
            sublist_insert,
            "visits",
            0,
        ),
    );

    // 6. ListRuin (LNS) — remove 2-5 visits from a random route
    let list_ruin = ListMoveListRuinSelector::new(ListRuinMoveSelector::<VrpPlan, usize>::new(
        2,
        5,
        entity_count,
        list_len,
        ruin_remove,
        ruin_insert,
        "visits",
        0,
    ));

    let combined = UnionMoveSelector::new(
        UnionMoveSelector::new(
            UnionMoveSelector::new(list_change, list_swap),
            UnionMoveSelector::new(list_reverse_sel, sublist_change),
        ),
        UnionMoveSelector::new(kopt_sel, list_ruin),
    );

    let initial_score = director.calculate_score();
    let initial_solution = director.clone_working_solution();

    let mut scope: SolverScope<VrpPlan, _> = SolverScope::new(director);
    scope.set_time_limit(Duration::from_secs(time_limit_secs));
    scope.set_best_solution(initial_solution, initial_score);
    scope.start_solving();

    let mut phase: LocalSearchPhase<
        VrpPlan,
        ListMoveImpl<VrpPlan, usize>,
        _,
        LateAcceptanceAcceptor<VrpPlan>,
        AcceptedCountForager<VrpPlan>,
    > = LocalSearchPhase::new(
        combined,
        LateAcceptanceAcceptor::new(200),
        AcceptedCountForager::new(4),
        None,
    );

    phase.solve(&mut scope);

    scope.take_best_or_working_solution()
}

/// Total route cost across all vehicles.
pub fn compute_cost(plan: &VrpPlan) -> i64 {
    plan.vehicles
        .iter()
        .filter(|v| !v.visits.is_empty())
        .map(|v| {
            let data = unsafe { &*(v.data as *const ProblemData) };
            data.route_distance(&v.visits)
        })
        .sum()
}
