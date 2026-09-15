# Problem shape → surface

Classify each generated view by what the planning variable means in the real
world, then compose the stock components. These are the recurring shapes for
SolverForge apps; most domains are one or a mix of them.

The always-on shell is the same everywhere: header (tabs + solve/pause/resume/
stop/analyze), status bar bound to the header, one panel per view, a Data tab, a
REST API tab, a score-analysis modal, a footer. The shape only changes the
**primary surface**.

---

## 1. Time-window assignment (shift rostering, bookings, dispatch)

**Signals:** scalar variable on a time-bearing entity (`shift.start`/`shift.end`,
`booking.slot`), assigning to a person/resource/slot; `allowsUnassigned: true`.
**Primary surface:** rail timeline.

```
axis: day-aligned horizon over the roster period
lanes: one per assignee (employee) and optionally one per location
       + a trailing "Unassigned" lane
items: one per assigned entity in its lane, startMinute/endMinute from the record
overlays: per-assignee availability / preferences (see below)
badges: shift count, skill mismatch, infeasible
stats: hours, assignments
```

- Overlays express the fact side of the model: for each employee,
  `unavailableDates → red`, `undesiredDates → amber`, `desiredDates → emerald`
  as `{ dayIndex, dayCount: 1, label, tone }` bands.
- A second view (`by location`) is a re-grouping of the same records — same
  builder, different lane key.
- Mimic: `solverforge-usecases/uc-hospital`.

## 2. Discrete timetabling (slot grid)

**Signals:** scalar variable mapping a lesson/meeting to a `timeslot` fact
(day + period) and a `room`/`teacher` fact.
**Primary surface:** rail timeline with an explicit day axis.

```
axis: days[] from the distinct day-of-week values; ticks per period/hour
lanes: one per group, room, or teacher (offer all three as tabs)
items: lessons at dayIndex*1440 + clock; tone by subject
badges: "n/m scheduled" green when a group is fully placed, amber otherwise
empty lanes: still rendered, so unplaced capacity is visible
```

- Three groupings (by group / by room / by teacher) over the same records is the
  canonical presentation; the user switches tabs.
- Mimic: `solverforge-usecases/uc-lessons`.

## 3. Ordered sequence / route (list variable, time-bearing)

**Signals:** `list` variable (`visits[]`, `deliveryOrder[]`) on a vehicle/route
entity that carries start/end times, plus element facts with service durations
and time windows.
**Primary surface:** rail timeline, one lane per route; a map if geospatial.

```
axis: service day or multi-day horizon
lanes: one per vehicle/route (+ route badges: capacity, skills, late, overtime)
items: replay the sequence (travel from the leg matrix, wait for the window,
       service) to get startMinute/endMinute per stop
stats: stops, travel, service time
tone: per-route palette index, or backend-supplied route colour
```

- Replay is required: list variables store order, not times.
- Add a per-delivery lane view to show each element's window (a slate "Window"
  item) and whether a vehicle was assigned.
- Mimic: `solverforge-usecases/uc-deliveries` (timeline + map), `uc-fsr`
  (timeline + map + route cards).

## 4. Ordered sequence without time (batching, ordering)

**Signals:** `list` variable with no time fields (bins, batches, picking order).
**Primary surface:** rail timeline with a synthetic slot axis, or ordered lists.

```
axis: one slot per element position (slotMinutes = 60, "Window" day groups)
lanes: one per container/bin; items are elements in sequence order
       + an "Empty" badge for unused containers
stats: items, first, last
```

The scaffold's generic list view already does this; replace it when a domain
label reads better.

## 5. Resource loading / machine scheduling with changeovers

**Signals:** job entity with start/end on a machine/furnace/resource, plus setup
between consecutive jobs.
**Primary surface:** low-level rail primitives when you need gauges/changeovers,
otherwise `createTimeline`.

```
SF.rail.createHeader({ label, labelWidth, columns })
SF.rail.createCard({ id, name, type, badges, gauges, stats })   // per resource
card.addBlock({ start, end, horizon, label, meta, color, borderColor })
SF.rail.addChangeover(card.rail, { start, end, horizon })
card.setUnassigned([...]); card.setSolving(true)
```

- Gauges (`heat`, `load`, `emerald`) show utilization/temperature/load.
- Keep `minWidthPct: 0.5` default for visibility, or `0` when exact interval
  geometry matters.
- This is the shape to use when resource identity and load matter more than a
  calendar grid.

## 6. Precedence-constrained project scheduling

**Signals:** task entities with duration, dependencies, assignable resources.
**Primary surface:** Gantt (`SF.gantt.create`).

```
tasks: [{ id, name, start, end, priority, projectIndex, dependencies, custom_class }]
columns: task/start/end/priority with sortable headers
onDateChange: your re-plan hook
```

- Use `viewMode` (`Day`/`Week`/`Month`) for horizon, `highlightTask` to focus a
  critical task, `projectIndex` for colour.
- Gantt implies user editing; if the surface is read-only, the rail timeline
  reads better for dense schedules.

## 7. Geospatial routing

**Signals:** locations with lat/lng, routes with ordered stops, backend route
geometry.
**Primary surface:** map module, with the timeline and route cards alongside.

```
map: vehicle markers, visit markers (gray when unassigned), encoded polylines
side: route cards (stops, distance, duration, tags) with highlight → dim others
below: timeline per vehicle
guard: geometry scoped to {jobId, snapshotRevision}
```

- Load Leaflet + `sf-map.js`/`sf-map.css`.
- Dim non-focused routes rather than hiding them.
- Mimic: `uc-deliveries`, `uc-fsr`.

## 8. Numeric scalar assignment / countable ranges

**Signals:** scalar variable over `countableRange` (`hour`, `bay`, `level`), or
an index into a numeric fact collection.
**Primary surface:** synthetic-slot timeline lane per entity, or tables/KPI.

```
axis: slots for each value in [from, to)
lanes: one per entity with a numeric variable (or group by value)
items: place the entity at (value - from) * slotMinutes
```

Use `.sf-kpi-card` for aggregate signals (total, max, imbalance). For
"assign a number" problems (quantities, staffing levels), prefer tables plus a
distribution view; a timeline of numbers is only clear when position is the
point.

## 9. Pure assignment (no time, no geo)

**Signals:** scalar assignment to a categorised fact (`category`, `color`,
`group`) with no time or coordinates.
**Primary surface:** assignment matrix as lanes, or a grouped table.

```
lanes: one per target category/group (all targets, even empty)
items: one per assigned entity, ordered by a stable key
tone: per target/tone-hash
```

Add KPI cards for counts per group and the constraint dots for feasibility.

## 10. Mixed models

Real apps combine shapes: a route model with time windows (3 + 1), a roster with
skills and preferences (1 + 9). Compose surfaces, one tab each:

- one tab per meaningful grouping of each variable,
- a map tab only if the model is geo,
- one shared Data tab and one shared analysis modal,
- one solver controller and one status bar for the whole app.

Do not create one solver per tab — there is exactly one retained job.

---

## Choosing between timeline, Gantt, and rail primitives

| Need | Use |
| --- | --- |
| Dense time-scaled read-only schedule, many lanes, exact geometry | `SF.rail.createTimeline` |
| Editable tasks with dependencies and drag/reorder | `SF.gantt.create` |
| Resource identity with gauges/load and explicit changeovers | `SF.rail.createCard` + `addBlock` + `addChangeover` |
| Assignment with no time axis | timeline synthetic slots, or tables |
| Geography | `SF.map.create` (+ timeline alongside) |
