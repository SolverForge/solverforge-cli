# Tutorial: Cloud Server Placement Optimizer

**Build a constraint solver that assigns workloads to data center servers — in under 5 minutes.**

Most cloud teams still use spreadsheets or gut feel to decide which server runs which workload. SolverForge replaces that with a real constraint solver — the same class of technology behind Google OR-Tools and OptaPlanner — but scaffolded in seconds and running as a native Rust binary.

---

## What We're Building

A solver that assigns **workloads** (VMs, containers, batch jobs) to **servers** across a data center, respecting:

- **Hard**: No server exceeds its CPU or RAM capacity
- **Hard**: GPU workloads only land on GPU-equipped servers
- **Soft**: Spread workloads evenly across servers (avoid hotspots)
- **Soft**: Co-locate workloads that talk to each other frequently (network affinity)

This is a **bin packing + affinity** problem — the kind of thing that takes weeks to hand-code and minutes with SolverForge.

---

## Step 1: Scaffold the Project

```bash
solverforge new cloud-server-placement --basic
cd cloud-server-placement
```

```
▸ Creating basic project cloud-server-placement

  Done! Your project is ready.

  Next steps:
    cd cloud-server-placement
    $ solverforge server
    Open http://localhost:7860 in your browser
```

You already have a compiling Rust project with a solver, web API, and starter constraint. But the generic domain (Task/Resource) isn't ours yet. Let's replace it.

---

## Step 2: Build the Domain with the CLI

### Create the solution

```bash
solverforge add solution placement_plan
```

```
▸ Created src/domain/placement_plan.rs
▸ Updated src/domain/mod.rs
```

### Create the planning entity

```bash
solverforge add entity workload --planning-variable server_idx
```

```
▸ Created src/domain/workload.rs
▸ Updated src/domain/mod.rs
▸ Updated src/domain/placement_plan.rs   ← auto-wired with #[planning_entity_collection]
```

The CLI created the entity **and** wired it into your solution automatically. Open `src/domain/workload.rs` — the planning variable is already annotated:

```rust
#[planning_entity]
pub struct Workload {
    #[planning_id]
    pub id: String,
    #[planning_variable(allows_unassigned = true)]
    pub server_idx: Option<usize>,
}
```

Add your domain fields (`cpu_required`, `ram_required`, `needs_gpu`, `affinity_group`) to the struct. That's the only hand-editing for the entity.

### Create the problem fact

```bash
solverforge add fact server
```

```
▸ Created src/domain/server.rs
▸ Updated src/domain/mod.rs
▸ Updated src/domain/placement_plan.rs   ← auto-wired with #[problem_fact_collection]
```

Add your fields (`cpu_capacity`, `ram_capacity`, `has_gpu`, `rack`) to the generated `Server` struct.

**That's 3 CLI commands to build the entire domain model.** The solution struct, entity collection wiring, fact collection wiring, module declarations — all automatic.

---

## Step 3: Add Constraints with the Wizard

### Capacity constraint (hard)

```bash
solverforge add constraint capacity_limit
```

The interactive wizard walks you through it:

```
? Constraint type
  ❯ Penalize matching entities        (e.g. unassigned, invalid state)
    Penalize conflicting pairs        (e.g. overlapping shifts, double-booking)
    Penalize entity-fact mismatch     (e.g. missing skill, wrong location)
    Balance assignments               (e.g. fair workload distribution)
    Reward matching entities          (e.g. desired day, preferred shift)
```

Select **"Penalize entity-fact mismatch"** (workloads vs. server capacity), then **Hard**.

```
▸ Created src/constraints/capacity_limit.rs
▸ Updated src/constraints/mod.rs
```

The CLI generates a constraint skeleton with the correct types, imports, and ConstraintFactory chain. Open the file — you just fill in the TODO filter:

```rust
// Generated skeleton — just replace the TODO:
.join(
    |s: &PlacementPlan| s.servers.as_slice(),
    equal_bi(
        |w: &Workload| w.server_idx,
        |s: &Server| todo!("return matching key from Server"),  // ← your edit
    ),
)
.filter(|w: &Workload, s: &Server| {
    todo!("add mismatch condition")  // ← your edit
})
.penalize(HardSoftScore::ONE_HARD)
```

### GPU requirement (hard) — one-liner with flags

Skip the wizard entirely:

```bash
solverforge add constraint gpu_required --join --hard
```

```
▸ Created src/constraints/gpu_required.rs
▸ Updated src/constraints/mod.rs
```

Same skeleton, same TODO pattern. Fill in the GPU check.

### Load balancing (soft) — zero config

```bash
solverforge add constraint balance_load --balance
```

```
▸ Created src/constraints/balance_load.rs
▸ Updated src/constraints/mod.rs
```

The `--balance` flag auto-selects soft and generates the balance pattern:

```rust
// Generated — works out of the box, just set the grouping key:
.for_each(|s: &PlacementPlan| s.workloads.as_slice())
.balance(|w: &Workload| w.server_idx)
.penalize(HardSoftScore::of_soft(1))
```

### Network affinity (soft)

```bash
solverforge add constraint network_affinity --reward
```

```
▸ Created src/constraints/network_affinity.rs
▸ Updated src/constraints/mod.rs
```

Fill in the reward condition: same affinity group + same server.

---

## Step 4: Run It

```bash
solverforge server
```

Open `http://localhost:7860`. Hit **Solve**. Watch 50 workloads land on 10 servers without a single capacity violation — in seconds.

---

## Recap: What the CLI Did For You

| Step | CLI Command | What it auto-generated |
|------|------------|----------------------|
| Solution | `solverforge add solution placement_plan` | Solution struct, score field, mod.rs |
| Entity | `solverforge add entity workload --planning-variable server_idx` | Entity struct, planning variable annotation, **wired into solution** |
| Fact | `solverforge add fact server` | Fact struct, **wired into solution** |
| Constraint 1 | `solverforge add constraint capacity_limit` | Join skeleton, **registered in mod.rs** |
| Constraint 2 | `solverforge add constraint gpu_required --join --hard` | Join skeleton, registered |
| Constraint 3 | `solverforge add constraint balance_load --balance` | Balance skeleton, registered |
| Constraint 4 | `solverforge add constraint network_affinity --reward` | Reward skeleton, registered |

**8 commands. Zero manual wiring. You only hand-edit domain fields and constraint TODOs.**

---

## Why This Matters

Cloud providers charge you for idle capacity. Internal platforms waste hardware. This solver finds placements that are **feasible** (no overloads) and **optimal** (balanced, network-aware) — something no human can do at scale.

Ship it as a microservice. Call it from your provisioning pipeline. Replace the spreadsheet.

**Total time from zero to working solver: ~5 minutes.**

---

## Next Steps

- Add a **"no single point of failure"** constraint: `solverforge add constraint rack_spread --pair --hard`
- Add a **migration cost** soft constraint: `solverforge add constraint migration_cost --unary --soft`
- Swap score type: `solverforge add score HardMediumSoftScore`
- Feed real inventory from your CMDB via the CSV import or API endpoint
