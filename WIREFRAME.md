# solverforge-cli Wireframe

## Scaffolding Surface

The standalone CLI currently ships one built-in scaffold path only:

- `solverforge new <name>`

Domain-specific examples such as employee scheduling and vehicle routing are intentionally out of scope for the built-in scaffold catalog. They belong in quickstarts.

That built-in scaffold is a neutral shell. Users shape it afterward through facts, entities, variables, constraints, generated data, and `solverforge.app.toml` instead of selecting a starter family up front.

## Frontend Rule

For any generated frontend feature, ask first: does `solverforge-ui` already provide it?

If yes, the scaffold should use the shipped `solverforge-ui` surface instead of re-implementing it. In practice that means the generated apps should rely on:

- `solverforge_ui::routes()` for `/sf/*` assets
- `SF.createBackend()` and `SF.createSolver()` for solver lifecycle
- `SF.createHeader()`, `SF.createStatusBar()`, `SF.createModal()`, `SF.createApiGuide()`, `SF.createFooter()`
- `SF.createTable()` plus view-specific render glue
- `SF.rail.*`, `SF.gantt.*`, or optional modules only when the scaffold actually needs them

The scaffold may still own thin composition code for domain-specific projections that are not yet shipped as turnkey `solverforge-ui` views.

## Generated Project Shape

The built-in neutral scaffold should generate:

- `Cargo.toml` with published `solverforge`, `solverforge-ui`, and `solverforge-maps` dependencies
- `solverforge.app.toml` as the scaffolded app/domain contract
- `src/api/` exposing the retained `/jobs` REST/SSE contract expected by `solverforge-ui`
- `static/index.html` loading `/sf/sf.css` and `/sf/sf.js`
- `static/app.js` containing only app composition and view-specific rendering
- `static/generated/ui-model.json` as the compiler-owned view/model projection
- `static/sf-config.json` as the preserved customization seam

## `sf-config.json`

`sf-config.json` remains the generated UI wiring file. CLI updates must preserve:

- unknown top-level keys
- unknown `view` shapes

The CLI may extend the config only for scaffold-owned rendering concerns. It should not duplicate configuration for behaviors already covered by `solverforge-ui` primitives.

## RC Defaults

`.solverforgerc` still carries a legacy `default_template` field in the parser for compatibility with older configs and tests, but it is not part of the current public scaffold surface and does not define multiple starter families today.

Do not extend docs or new features around `default_template` unless the public scaffold model changes again.
