# Mapping the planning model onto a surface

The work is a pure transform: plan JSON in, component config out. Write it as
small functions with no DOM access so you can unit-test it. Nothing here assumes
a domain; substitute your own field names.

## 0. Read the real shape first

From `/demo-data/<id>` (a plan) and a `/jobs/{id}/snapshot` (a solution):

- Which array is the **planning entity** (what gets assigned)?
- Which field on it is the **planning variable** (`employeeIdx`, `room_idx`,
  `visits[]`, `deliveryOrder[]`, `hour`)?
- For scalar variables, the value indexes into a **fact** array
  (`sourcePlural`) or is a number in a `countableRange`.
- For list variables, the value is an ordered array of element ids/indexes, and
  the entity may also carry route metadata (start/end, capacity, home).
- Which fields carry **time** (`start`, `end`, `earliest`, `latest`,
  `duration`), in what unit and format?
- Which fields are **human labels** to show verbatim (`name`, `label`, `kind`)?

Never rename these. If the model says `employeeIdx`, the UI says "Employee".

## 1. Normalize time to integer minutes

`SF.rail.createTimeline` is integer-minute only and rejects strings, `Date`s,
numeric strings, and fractional minutes. Convert once, at the boundary. The four
conventions seen in shipped apps:

**a. Backend `NaiveDateTime` string → timezone-free epoch ms → minutes**
(hospital). Parse the wall-clock components with `Date.UTC` so the browser never
injects a local zone, then diff against a horizon start.

```js
const MINUTE_MS = 60_000;
export function parseDateTimeMs(value) {
  const s = String(value || '').trim().replace(' ', 'T');
  const m = s.match(/^(\d{4})-(\d{2})-(\d{2})(?:T(\d{2}):(\d{2})(?::(\d{2}))?)?/);
  if (!m) return null;
  return Date.UTC(+m[1], +m[2] - 1, +m[3], +(m[4] || 0), +(m[5] || 0), +(m[6] || 0));
}
const startOffset = Math.max(0, Math.round((row.startMs - axis.horizonStartMs) / MINUTE_MS));
```

**b. Seconds since midnight → minutes** (deliveries). Floor the start, ceil the
end so the rendered block covers the whole service.

```js
startMinute: Math.floor(stop.serviceStartTime / 60),
endMinute:   Math.ceil(stop.departureTime / 60),
```

**c. Day-of-week + `HH:MM:SS` → minutes** (lessons). Lay days end to end with
`dayIndex * 1440 + clockMinutes`; clamp to `0..1439`.

```js
const dayIndex = DAY_MAP[slot.day_of_week];        // Mon=0 … Sun=6
startMinute: dayIndex * 1440 + parseTimeToMinutes(slot.start_time),
endMinute:   dayIndex * 1440 + parseTimeToMinutes(slot.end_time),
```

**d. Domain already in minutes** (fsr). Use directly; replay routes to derive
service windows.

```js
let clock = route.shift_start_minute;
for (const visitIdx of route.visits) {
  const visit = visits[visitIdx];
  clock += legMinutes(leg);
  if (clock < visit.earliest_minute) clock = visit.earliest_minute;
  entries.push({ start: clock, end: clock + visit.duration_minutes });
  clock += visit.duration_minutes;
}
```

Convert a duration to minutes with `Math.ceil(seconds / 60)` and validate that
every emitted minute is an integer before it reaches the timeline.

## 2. Build the axis

```js
axis: {
  startMinute: 0,                       // integer
  endMinute: horizon * 1440,            // integer; day-aligned when possible
  days: [ { id, label, subLabel?, startMinute, endMinute, isWeekend? } ],
  ticks: [ { id, minute, label } ],     // 2h/6h/1day spacing by zoom
  initialViewport: { startMinute, endMinute },
}
```

- Day-aligned horizons read best: horizon start = UTC midnight of the earliest
  item, end = midnight after the latest.
- `days[]` labels are the date/week label; `ticks[]` are the time grid.
- `isWeekend: /^(Sat|Sun)\b/.test(label)` gives weekend shading.
- `initialViewport` = one day / first teaching window / first 14 days depending
  on horizon. For fixed-horizon surfaces, pass `zoomPresets: []` so the user
  cannot zoom away from the intended window.
- For non-temporal domains (sequence positions, slots), build a synthetic axis:
  one slot = 60 minutes, `days[]` in groups ("Window 1"), `ticks[]` per slot.
  This is how the scaffold renders scalar/list views with no timestamps.

## 3. Build lanes

One lane per real grouping: assignee, resource, location, vehicle, group, room,
teacher. Lane fields: `id`, `label`, `mode`, optional `badges`, `stats`,
`overlays`, `items`.

- **Keep empty entities as lanes.** An employee/room with no assignment must
  still appear; otherwise the user cannot see slack. Mark it `badges: ['Empty']`.
- **Group by the planning variable's meaning.** For an employee assignment,
  group entity rows by assigned employee. For a location model, by location.
- **Stable ids.** `id: 'employee-' + item.id` or a normalized name. Never use a
  display label as an id.
- **`stats`** are small lane-level facts: `{ label: 'Items', value: n }`,
  first/last, totals. **`badges`** are short status strings or
  `{ label, style }` objects (`['Empty']`, `['Needs assignment']`, `['Longest']`).
- **`mode: 'detailed'`** for per-assignment inspection (one row of timeline
  blocks), **`mode: 'overview'`** for scanable aggregate lanes. Overview items
  can carry `summary` and expandable `detailItems` with a shared `clusterId`.

## 4. Build items

```js
{
  id: laneId + '-item-' + index,     // unique across the timeline
  startMinute: 360, endMinute: 1080, // integers, end > start
  label: item.name,                  // the model's own label
  meta: 'Room 4 · Ada',              // string, {k:v}, or [{label,value}]
  tone: 'blue',
  summary: undefined,                // overview mode only
  clusterId: undefined,              // group overview items that expand together
  detailItems: undefined,            // concrete items behind an overview block
}
```

- `label` is the primary text; `meta` is the secondary line/tooltip. Both accept
  the model's strings directly. `meta` may be a string, an object, or an array of
  `{ label, value }` rows (used for rich hover detail).
- Set `endMinute > startMinute`; for zero/instantaneous work use a minimum span
  (e.g. `start + 1`) so the block is visible and exact.
- Do not mutate the source plan; build new item objects.

### Overview summary (for `mode: 'overview'`)

```js
summary: {
  primaryLabel: 'Monday intake surge',
  secondaryLabel: 'ER · trauma · overflow',
  count: 24,           // optional explicit total
  openCount: 3,        // optional explicit open/unassigned
  toneSegments: [ { tone: 'blue', count: 15 }, { tone: 'rose', count: 3 } ],
}
```

Summary fields are additive: if you give `count` but no `openCount`/`toneSegments`
and the library cannot inspect the backing items, those stay unknown and are not
rendered — supply them explicitly when you want them shown. Give overlapping
overview items a shared `clusterId` if you want programmatic
`expandCluster(laneId, clusterId)`. Each lane may produce at most one overview
group per `clusterId`.

## 5. Tones

Available: `emerald, blue, amber, rose, violet, cyan, red, slate` (or a raw CSS
colour / `{background,border,overlay,text}` object).

Three strategies, all domain-agnostic:

```js
// 1. Hash a stable key (subject, category) into the palette
function toneForKey(key) {
  const palette = ['emerald','blue','amber','rose','violet','slate'];
  let hash = 0; const text = String(key || '');
  for (let i = 0; i < text.length; i += 1) hash = ((hash * 31) + text.charCodeAt(i)) >>> 0;
  return palette[hash % palette.length];
}
// 2. Map a known enum to an explicit tone
function toneForKind(kind) {
  return { business: 'blue', residential: 'emerald', restaurant: 'amber' }[kind] || 'slate';
}
// 3. Index by lane/route order
const PALETTE = ['blue','emerald','amber','rose','violet','slate'];
const tone = PALETTE[routeIndex % PALETTE.length];
```

Use `red` deliberately for unassigned or infeasible, `emerald` for feasible or
complete. Reserve one tone per meaning within a surface.

## 6. Surface unassigned work

Unassigned assignments are first-class and must be visible:

- **Extra lane**: append a lane `{ id: 'unassigned', label: 'Unassigned',
  badges: ['Needs assignment'], mode: 'detailed', items: [...] }` (hospital,
  lessons). Best for time-bearing work.
- **Badge + marker**: mark the lane/row `Unassigned` and color the map marker
  gray (`#9ca3af`) with no route (deliveries).
- **Separate card/table**: list unassigned elements in their own table with
  their windows (fsr).

Never hide an `allowsUnassigned` variable with a null value.

## 7. Lanes for non-time assignments

When the variable has no time dimension, the timeline still works as a
"placement" view: one lane per target fact, items ordered left to right with a
synthetic slot axis (one slot per element). Alternatively use tables and KPI
cards:

- Assignment matrix: lane per target, item per assigned entity.
- Countable range: items placed at `value * slotMinutes`.
- Pure counts/packing: `SF.createTable` + KPI cards (`.sf-kpi-card`).

## 8. Timeline instance management

Two valid patterns:

- **Create once, `setModel` per render** (lessons, deliveries, fsr): keeps
  viewport/expansion stable across solver updates. Best for live solving.
- **Recreate per render then `setViewport`** (hospital): simpler when the lane
  set changes shape; you must re-apply the viewport.

Always `destroy()` timelines on `beforeunload`, and when a view becomes empty,
so detached listeners/layout observers do not leak.

## 9. Gantt mapping

Use `SF.gantt.create` when tasks have precedence and the user should drag to
re-plan. Task shape:

```js
{
  id: String(task.id),
  name: task.label,
  start: '2026-03-15 09:00',        // 'YYYY-MM-DD HH:mm' strings
  end:   '2026-03-15 10:30',
  priority: 1,
  projectIndex: 0,                   // project colour
  pinned: true,                      // pinned task styling
  custom_class: 'project-color-0 priority-1',
  dependencies: 'task-1',           // comma-separated ids
}
```

Provide `columns[]` (`{ key, label, sortable, render }`) for the grid; `render`
returns `{ unsafeHtml }` only with escaped content. `changeViewMode` accepts
`Quarter Day`…`Month`. `onDateChange(task, start, end)` is your edit hook.

## 10. Map mapping (routing)

```js
var map = SF.map.create({ container: 'map', center: [lat, lng], zoom: 12 });
map.clearAll();
vehicles.forEach(v => map.addVehicleMarker({ lat: v.lat, lng: v.lng, color: colorForVehicle(v.id) }));
deliveries.forEach(d => map.addVisitMarker({
  lat: d.lat, lng: d.lng, color: d.assigned ? colorForVehicle(d.vehicleId) : '#9ca3af',
  icon: iconForKind(d.kind),
}));
routes.forEach(seg => map.drawEncodedRoute({ encoded: seg.encodedPolyline, color, opacity, weight }));
map.fitBounds();                       // only when the location signature changes
```

- Route geometry should come from the backend (`/jobs/{id}/routes?...`), scoped
  to `{jobId, snapshotRevision}` as in `references/solver-lifecycle.md`. Do not
  fake geometry from straight lines unless the model has no road data.
- Draw only segments the backend marks usable (`ROUTED` and `reachable`).
- Dim unfocused routes (opacity `0.2`, weight `2`) and highlight focus
  (`1`/`5`); `fitBounds()` only when the set of coordinates actually changes.

## 11. Tables and KPI cards

- Raw model tables: one section per `ui-model.json` entity/fact, columns from the
  record keys, arrays joined, objects stringified. This is the Data tab.
- Analysis table columns: Constraint / Type / Score / Matches, reading
  `analysis.constraints[]` (`name`, `constraintType|type`, `score`,
  `matchCount|matches.length`).
- Summary row above a timeline: `SF.createTable` with the domain's headline
  metrics (counts, longest sequence, average, empties).

## 12. Handle partial data

Solve events can arrive before the plan is complete, and demo data can fail.
Always: guard `snapshot.solution`, return `null` from builders when required
collections are empty, render an explicit empty message, and keep the previous
render rather than clearing to a blank page. Demo-data failures get an inline
banner and a disabled Solve button, not a crash.
