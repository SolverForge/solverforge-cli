# Component catalog (`SF.*`)

All factories are global on `SF`. Text arguments are rendered as text unless a
field is explicitly named `unsafeHtml`/`unsafeBody`. Config objects are asserted;
a wrong shape throws early rather than failing silently.

## Shell components

### `SF.createHeader(config) -> HTMLElement`

Sticky header: logo, brand text, nav tabs, action buttons.

- `logo` (url), `title`, `subtitle`
- `tabs: [{ id, label, icon?, active? }]` — `icon` is a Font Awesome name like
  `fa-table-cells-large` (rendered as `fa-solid <icon>`). Keyboard arrows move
  between tabs.
- `onTabChange(id)` — fires on click; you switch panels.
- `actions: { onSolve?, onPause?, onResume?, onCancel?, onAnalyze? }` — only the
  callbacks you pass produce buttons. Solve (`success`, play), Pause (`default`),
  Resume (`primary`), Stop (`danger`), Analyze (`ghost` circle chart icon).
  Pause/Resume/Stop start hidden; the status bar reveals them by lifecycle.
- The returned element carries `header.sfControls` (spinner, buttons, nav);
  `statusBar.bindHeader(header)` uses it. Do not create your own solve buttons.

### `SF.createStatusBar(config) -> api`

Score display, constraint dots, moves/s, lifecycle text — and it drives the
header's action visibility.

- `constraints: [{ name, type: 'hard'|'soft'|'medium' }]`
- `header` (or call `bindHeader(header)`), `onConstraintClick(index)`
- Returns `{ el, bindHeader, updateScore, setLifecycleState, setSolving,
  updateMoves, updateConstraintDots, colorDotsByScore, colorDotsFromAnalysis }`.
- `createSolver({ statusBar })` calls `updateScore`, `setLifecycleState`,
  `updateMoves`, and `colorDotsFromAnalysis` for you. Bind the header before
  starting solves.

### `SF.createFooter(config) -> HTMLElement`

`{ links: [{ label, url }], version? }`. Links open in a new tab.

### `SF.createApiGuide(config) -> HTMLElement`

`{ endpoints: [{ method, path, description?, curl? }] }`. Renders each endpoint
with a copy button for the curl example. Put it on the `api` tab.

### `SF.createModal(config) -> { el, body, open, close, setBody }`

`{ title, width?, onClose? }`; Escape closes; focus is restored. `setBody`
renders a string as text, a Node, or `{ unsafeBody }`/`{ unsafeHtml }`. Build
DOM for anything with data in it.

### `SF.createTabs(config) -> { el, show }`

`{ tabs: [{ id, content, active? }] }`. `content` may be a string (text), a
Node, or `{ unsafeHtml }`. `SF.showTab(id, root?)` activates matching panels
globally or within `root`. Most SolverForge apps use header nav + hidden panels
instead of this; either is valid.

## Display components

### `SF.createTable(config) -> HTMLElement`

- `columns`: strings or `{ label, align?, width?, className? }`
- `rows`: array of arrays; a cell may be a string/number (text), a Node, or
  `{ unsafeHtml }`
- `onRowClick(rowIndex, row)`

The wrapper is `.sf-table-container`; the table is `.sf-table`.

### `SF.createButton(config) -> HTMLButtonElement`

`{ text?, icon?, variant?, size?, pill?, circle?, outline?, iconOnly?, tooltip?,
disabled?, onClick? }`.

Variants: `success` (white bg, emerald text), `danger`, `primary` (emerald-700),
`default` (gray border), `ghost`. Sizes: `small`, `large`. Example set in the
README's Button Variants section.

### `SF.showToast(config)` / `SF.showError(title, detail)`

Toasts auto-dismiss; `variant` defaults to `danger`. Use `showError` for
runtime/solver failures and a toast for lighter confirmations.

## Solver lifecycle

### `SF.createBackend(config) -> backend`

`{ type: 'axum'|'tauri'|'fetch', baseUrl?, jobsPath?, demoDataPath?, headers? }`
(HTTP); Tauri also needs `invoke`, `listen`, `eventName?`. See
`references/solver-lifecycle.md` for endpoint and method contracts.

### `SF.createSolver(config) -> api`

Returns `{ start, pause, resume, cancel, delete, getSnapshot, analyzeSnapshot,
isRunning, getJobId, getLifecycleState, getSnapshotRevision }`. Full contract in
`references/solver-lifecycle.md`.

## Rail scheduling

### `SF.rail.createTimeline(config) -> { el, setModel, setViewport, expandCluster, destroy }`

The canonical read-only scheduling surface. Config:

- `title?`, `subtitle?`, `label?` (corner label, default `Lane`)
- `labelWidth?` (px, default `280`; compacted automatically in narrow embeds)
- `zoomPresets?` — subset of `['1w','2w','4w','reset']`, default all four;
  `[]` removes zoom controls for fixed-horizon surfaces
- `model` — the normalized integer-minute model (see `references/data-mapping.md`)

Model: `axis { startMinute, endMinute, days[], ticks[], initialViewport }`,
`lanes[] { id, label, mode: 'overview'|'detailed', badges?, stats?, overlays?,
items[] }`, `items[] { id, startMinute, endMinute, label, meta?, summary?, tone?,
clusterId?, detailItems? }`, `overlays[] { dayIndex, dayCount?, label?, tone? }`
or `{ startMinute, endMinute, … }`.

Exact geometry: adjacent intervals stay disjoint on one track; true overlaps
pack onto separate rows. Detached timelines (created before `el` is mounted)
resynchronize layout after mount. `setModel` re-renders; `setViewport` pans.

### `SF.rail.createHeader(config)`, `SF.rail.createCard(config)`

Low-level primitives for custom furnace-style resource rails.
`createCard` → `{ el, rail, addBlock, clearBlocks, setSolving, setUnassigned }`
with `gauges` (`heat`/`load`/`emerald`), `stats`, `badges`, `type`.
`SF.rail.addBlock(rail, { start, end, horizon, label, meta?, color?,
borderColor?, minWidthPct? })`; `SF.rail.addChangeover(rail, {start,end,horizon})`;
`SF.rail.createHeatmap(config)`; `SF.rail.createUnassignedRail(tasks, onClick)`.

## Gantt (`SF.gantt.create`)

Requires the Frappe Gantt + Split.js vendor scripts and CSS. Returns
`{ el, mount(target), setTasks(tasks), refresh, getChart, changeViewMode(mode),
highlightTask(id), destroy }`.

Config: `gridTitle`, `chartTitle`, `viewMode` (`Quarter Day`|`Half Day`|`Day`|
`Week`|`Month`), `splitSizes`, `columns[] { key, label, sortable?, render? }`,
`onTaskClick(task)`, `onDateChange(task, start, end)`, `unsafePopupHtml?`.

Task: `{ id, name, start: 'YYYY-MM-DD HH:mm', end: 'YYYY-MM-DD HH:mm',
priority?, projectIndex?, pinned?, custom_class?, dependencies?, …fields }`.
Use `projectIndex`/`custom_class` for project colours and
`custom_class: 'project-color-N priority-M'` for badges.

## Map module (optional)

Load Leaflet + `/sf/modules/sf-map.js` + `sf-map.css`, then
`SF.map.create({ container, center: [lat,lng], zoom })`. Returns an instance with
`addVehicleMarker`, `addVisitMarker`, `addStopNumber`, `drawRoute({points})`,
`drawEncodedRoute({encoded, color, opacity?, weight?})`, `fitBounds`,
`highlight(color)`, `clearHighlight`, `clearRoutes`, `clearStops`,
`clearMarkers`, `clearAll`. Also `SF.map.decodePolyline(str)`.

## Utilities

| Function | Purpose |
| --- | --- |
| `SF.score.parseHard/parseSoft/parseMedium(str)` | read components from `"0hard/-42soft"` |
| `SF.score.getComponents(str)` | `{ hard, medium, soft }` |
| `SF.score.colorClass(str)` | `score-green` / `score-yellow` / `score-red` |
| `SF.colors.pick(key)` | stable Tango palette colour per key (cached) |
| `SF.colors.project(i)` | `{ main, dark, light }` from the 8-colour project palette |
| `SF.colors.reset()` | clear the colour cache |
| `SF.escHtml(str)` | HTML-escape |
| `SF.el(tag, attrs, ...children)` | DOM factory; `attrs.unsafeHtml`, `attrs.dataset`, `attrs.style`, `attrs.onClick`, `attrs.onKeyDown` |
| `SF.normalizeCreateJobId(raw)` | create-job response → string id or `''` |

## Timeline tone palette

`emerald, blue, amber, rose, violet, cyan, red, slate` — each resolves to a
`{background, border, text, overlay}` set. A raw CSS colour string is also
accepted (used as background/border, dark text). An object with
`background`/`border`/`overlay`/`text` keys overrides fully.

## Design tokens and type

- Colours: `--sf-emerald-500/600/700`, `--sf-red-600`, `--sf-amber-500`,
  `--sf-gray-50…900`; 8 project colours (emerald, blue, purple, amber, pink,
  cyan, rose, lime).
- Fonts: Space Grotesk (body/headings), JetBrains Mono (code/scores/data).
- Spacing `--sf-space-{0,1,2,3,4,5,6,8,10,12,16}`; shadows
  `--sf-shadow-{sm,base,md,lg,xl,2xl}` plus `--sf-shadow-emerald`.
- Animations: `sf-spin`, `sf-dot-pulse`, `sf-score-flash`,
  `sf-dialog-slide-in`, `sf-breathe`, `sf-slide-in`, `sf-fade-in`, `sf-late-glow`.
