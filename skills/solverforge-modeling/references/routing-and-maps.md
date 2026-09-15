# Routing with solverforge-maps

Use this when the problem has real geography and the plan quality depends on
road travel times, not straight-line distance. If distances are abstract
(sequence cost, machine changeover, compute time), keep the model simple and
skip this file.

## When maps matter

- Vehicle routing, field service, dispatch: cost is minutes/kilometres driven.
- Time windows and shift lengths depend on travel time between stops.
- Unreachable pairs are real feasibility constraints, not synthetic penalties.

If the model already carries a numeric distance/time matrix or a countable
range, do not add `solverforge-maps`.

## The crate

`solverforge-maps` (published 2.1.4; add it with `solverforge-maps = "2.1.4"`)
owns road networks, routing, and matrices. Key surface:

- `Coord::try_new(lat, lng)` and `BoundingBox::from_coords(...).expand_for_routing(...)`
- `NetworkConfig`, `SpeedProfile`
- `RoadNetwork::load_or_fetch(&bbox, &config, cache_dir).await` — OSM via Overpass, cached on disk
- `network.compute_matrix(&locations, None).await` → `TravelTimeMatrix` (all-pairs times + same-path route distances)
- `network.route(a, b)` → duration/distance/geometry for one pair
- `RouteResult` / route geometry with Douglas-Peucker simplification, polyline encoding
- `CacheStats`, `RoutingProgress`, connectivity analysis for unreachable diagnostics

See the `solverforge-maps` README for exact signatures and `docs/solverforge-maps/`
on the site for the pipeline and caching detail.

## Wiring it into a generated app

The web shell generates with `solverforge-maps` but with no routing data. A real
maps-backed app replaces the generated seed flow in `src/data/data_seed.rs`
with an async preparation step. The reference implementation is the FSR use case
(`solverforge-usecases/uc-fsr`, `solverforge-fsr@2.0.8`), which:

1. Collects the plan's locations.
2. Builds a bounding box and loads/fetches the road network into a stable cache
   such as `.osm_cache/<domain>/<area>`; never commit the cache.
3. Computes a `TravelTimeMatrix` and stores per-pair travel times on the model
   (a matrix field, or a distance-meter function keyed by indexes).
4. Exposes a route-geometry path for the UI: `GET /jobs/{id}/routes` returns
   snapshot-scoped geometry (encoded polylines), not straight lines, unless the
   model genuinely has no road data.

Keep data preparation out of constraint logic: constraints read the prepared
matrix through model methods or a distance meter, they do not call the network.

## Wiring road costs into a routing list variable

List variables model any ordered sequence — routes, ordered assignments, job
sequencing, precedence lists — and most of them never touch geography, so they
do not use this file at all. Use maps only when a list variable's ordering cost
is real road travel. Then the list variable reads its costs through the model:

- Stock VRP: `#[planning_list_variable(element_collection = "...", domain = "cvrp")]`.
  The `cvrp` profile owns the solution trait, distance meters, route/savings
  hooks, savings metric class, and strict route-local feasibility; it cannot be
  combined with the explicit hook flags.
- Custom routing: pass `--distance-meter`/`--intra-distance-meter`,
  `--route-hooks`, `--savings-hooks`, `--savings-metric-class-fn`,
  `--element-owner-fn`, `--precedence-duration-fn`, `--precedence-successors-fn`,
  and `--solution-trait` to `solverforge generate variable`. Each names a
  user-owned Rust implementation.

The CLI writes these into `solverforge.app.toml` and the web UI projection; you
still own the functions. After generating, confirm the model builds with
`solverforge check` and a real `cargo check`/solve.

## Route-derived shadows and geometry

When several rules need the same expensive route walk, compute it once and store
it as a shadow on the route entity. The FSR app does this with
`#[shadow_variable_updates(...)]` plus a `post_update_listener`, so constraints
read shadow fields (`route_unreachable_legs`, route distance, etc.) instead of
re-walking the matrix. Keep the scoring rules as stock `ConstraintFactory`
streams over the route rows; the shadow is the bridge, not a second engine.

## Shell and dependency caveats

- `web` is the only shell that generates `solverforge-maps` and `static/`.
- `api`, `cli`, and `mcp` shells exclude maps and the UI projection; a
  maps-backed MCP or API service must add `solverforge-maps` to its own manifest
  and prepare the routing data itself.
- Network fetches fail without connectivity; surface that as a bootstrap error
  rather than a solver failure, and cache aggressively for repeat runs.

## Verification

- Matrix shape matches the location count; unreachable pairs are counted, not
  hidden.
- A real solve reaches a scored terminal state (web/API) or the MCP tools return
  a scored snapshot; route geometry is present for routed snapshots.
- `CacheStats` shows the expected memory/disk/network split for repeated loads.
