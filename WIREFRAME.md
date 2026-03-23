# solverforge-cli Wireframe

## Scaffolding Surface

The standalone CLI ships two built-in scaffold families only:

- `solverforge new <name> --standard`
- `solverforge new <name> --list`

Domain-specific examples such as employee scheduling and vehicle routing are intentionally out of scope for the built-in scaffold catalog. They belong in quickstarts.

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

Both built-in scaffold families should generate:

- `Cargo.toml` with published `solverforge` and `solverforge-ui` dependencies
- `src/api/` exposing the REST/SSE contract expected by `solverforge-ui`
- `static/index.html` loading `/sf/sf.css` and `/sf/sf.js`
- `static/app.js` containing only app composition and view-specific rendering
- `static/sf-config.json` as the preserved customization seam

## `sf-config.json`

`sf-config.json` remains the generated UI wiring file. CLI updates must preserve:

- unknown top-level keys
- unknown `view` shapes

The CLI may extend the config only for scaffold-owned rendering concerns. It should not duplicate configuration for behaviors already covered by `solverforge-ui` primitives.

## RC Defaults

`default_template` values should align with the problem-type surface:

- `"standard"`
- `"list"`
