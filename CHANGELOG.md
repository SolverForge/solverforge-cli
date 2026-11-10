# Changelog

All notable changes to this project will be documented in this file. See [commit-and-tag-version](https://github.com/absolute-version/commit-and-tag-version) for commit guidelines.

## [4.0.3](https://github.com/SolverForge/solverforge-cli/compare/v4.0.2...v4.0.3) (2026-11-10)


### Bug Fixes

* **skills:** read smoke-test responses from files, not a curl pipe ([c78a799](https://github.com/SolverForge/solverforge-cli/commit/c78a7994bd48f6beb2a990e23fb0c9f03665af9f)), closes [#15](https://github.com/SolverForge/solverforge-cli/issues/15)


## [4.0.2](https://github.com/SolverForge/solverforge-cli/compare/v4.0.1...v4.0.2) (2026-11-10)


### Bug Fixes

* **scaffold:** target SolverForge 0.26.0 ([0c7c273](https://github.com/SolverForge/solverforge-cli/commit/0c7c2731fc948a16307a3b5bf6ea986ec6bae213)), closes [#13](https://github.com/SolverForge/solverforge-cli/issues/13)


## [4.0.1](https://github.com/SolverForge/solverforge-cli/compare/v4.0.0...v4.0.1) (2026-11-10)


### Bug Fixes

* **constraints:** list constraints from the constraint-modules manifest ([0902d68](https://github.com/SolverForge/solverforge-cli/commit/0902d687ef19c474b525650ef2fa80a32c156de7)), closes [#11](https://github.com/SolverForge/solverforge-cli/issues/11)


## [4.0.0](https://github.com/SolverForge/solverforge-cli/compare/v3.9.0...v4.0.0) (2026-11-10)


### ⚠ BREAKING CHANGES

* **scaffold:** Generated projects keep create_constraints and the constraint-calls block in src/constraints/assemble.rs, with each call written as `super::<id>::constraint()`; move them out of src/constraints/mod.rs before running generate, destroy or check.

* **scaffold:** move constraint assembly into src/constraints/assemble.rs ([73598ce](https://github.com/SolverForge/solverforge-cli/commit/73598cea7aec3300e81697b0b71e3c2df3656d71))


### Features

* **check:** reject code in mod.rs and lib.rs ([5b4c563](https://github.com/SolverForge/solverforge-cli/commit/5b4c563919032ec4e039caa0e077f5b1b78ca93f)), closes [#8](https://github.com/SolverForge/solverforge-cli/issues/8)
* **scaffold:** target SolverForge 0.25.1 ([a27ac8f](https://github.com/SolverForge/solverforge-cli/commit/a27ac8f6492d9f2f8bce436b518aec00cb7af4ed))


### Bug Fixes

* **check:** accept entity-scoped value_range_provider fields ([ae7defb](https://github.com/SolverForge/solverforge-cli/commit/ae7defbc7f94deebe4b6f519a68fa3c0d24a539a)), closes [#6](https://github.com/SolverForge/solverforge-cli/issues/6)


## [3.9.0](https://github.com/SolverForge/solverforge-cli/compare/v3.8.3...v3.9.0) (2026-11-09)


### Features

* **scaffold:** target SolverForge 0.25.0 ([e889443](https://github.com/SolverForge/solverforge-cli/commit/e8894435a4452cb2698fd964ecea6bdc262d69ab))


## [3.8.3](https://github.com/SolverForge/solverforge-cli/compare/v3.8.2...v3.8.3) (2026-11-07)


### Bug Fixes

* **scaffold:** target solverforge-ui 0.9.1, solverforge-maps 2.3.3 and rmcp 3.9.0 ([32f1969](https://github.com/SolverForge/solverforge-cli/commit/32f19697cc0ceadd9f8b8f151a0f746329c87a37))


## [3.8.2](https://github.com/SolverForge/solverforge-cli/compare/v3.8.1...v3.8.2) (2026-11-03)


### Bug Fixes

* **scaffold:** target solverforge-maps 2.1.5 ([3eda26b](https://github.com/SolverForge/solverforge-cli/commit/3eda26b3b7e04f113801ae429f20f65109bbe385))


## [3.8.1](https://github.com/SolverForge/solverforge-cli/compare/v3.8.0...v3.8.1) (2026-11-03)


### Bug Fixes

* **destroy:** name dependent files with forward slashes on every platform ([a68d432](https://github.com/SolverForge/solverforge-cli/commit/a68d4323ff32d9eff2136469a6424d55f7e906d2))
* **generate:** re-render only the data seed the CLI still owns ([b020746](https://github.com/SolverForge/solverforge-cli/commit/b020746205d3f3a1be346cb4e80dcfcadb410cd2))


## [3.8.0](https://github.com/SolverForge/solverforge-cli/compare/v3.7.1...v3.8.0) (2026-11-03)


### Features

* **scaffold:** target SolverForge 0.24.0 ([658064e](https://github.com/SolverForge/solverforge-cli/commit/658064e65cec7321e67398c73a0d679d45886b20))


### Bug Fixes

* **check:** state that `solverforge check` is structural and does not compile ([4762de4](https://github.com/SolverForge/solverforge-cli/commit/4762de4d9856305434b54f04227cdb387d4dd532))
* **constraints:** accept rustfmt-sorted constraint module lists ([615227b](https://github.com/SolverForge/solverforge-cli/commit/615227bd21dc41281b724211368ba1303d4fcf02))
* **destroy:** refuse to destroy types that source still references ([d9b54d8](https://github.com/SolverForge/solverforge-cli/commit/d9b54d822edf21926f0d8494ec9721ad0c47a062))
* **generate:** import --field types and add chrono when a field needs it ([9804ed7](https://github.com/SolverForge/solverforge-cli/commit/9804ed73d36ac7c923293c28ced2ac57d1791d4d))
* **generate:** import the bare name when rewriting a generic score type ([9c4b7f8](https://github.com/SolverForge/solverforge-cli/commit/9c4b7f8757cb3903f4c7ca10dbec7620ede724f8))
* **generate:** make unary, pair, and reward skeletons score as their help says ([128bfc5](https://github.com/SolverForge/solverforge-cli/commit/128bfc557802368b840857da213ba57a1872f310))
* **generate:** never let the rustfmt pass move managed block markers ([e0f7f3f](https://github.com/SolverForge/solverforge-cli/commit/e0f7f3f64f87a0e8be68bf1c4e849cb7376a858a))
* **generate:** reject --field names that duplicate generated fields ([ae8606e](https://github.com/SolverForge/solverforge-cli/commit/ae8606ef09c4b2406d944a241f1bf9f797bf3f76))
* **generate:** reject BendableScore as a planning solution score ([ec9eeda](https://github.com/SolverForge/solverforge-cli/commit/ec9eeda680139c993f6fd216a7edf04b52b9985e))
* **generate:** rustfmt the Rust sources generators and destroyers write ([1d130b1](https://github.com/SolverForge/solverforge-cli/commit/1d130b1a8c4f8adf90e75a4ff8c1a07ec6938fbc))
* **generate:** seed chrono Weekday and Utc fields without Default ([3ee96c3](https://github.com/SolverForge/solverforge-cli/commit/3ee96c3237e4be412fc4096924f276311b0802f2))
* **generate:** warn when a --field type is not defined in the domain ([393446c](https://github.com/SolverForge/solverforge-cli/commit/393446ce63a03ba3a3a2b85dd1ebf278f2f5abae))
* **routes:** list chained methods and multi-line route calls ([70a2983](https://github.com/SolverForge/solverforge-cli/commit/70a29836c3a55114ba8af2a105e609ebd4f5e2b9))
* **scaffold:** compile the CLI shell without dead-code warnings ([efbc593](https://github.com/SolverForge/solverforge-cli/commit/efbc593210a7acff0836b7723e7d84db73d336cc))
* **scaffold:** emit rustfmt-clean shell sources from `solverforge new` ([9e3807c](https://github.com/SolverForge/solverforge-cli/commit/9e3807c79e0f37934bd492f5de4bce97d674ea85))
* **scaffold:** let a fresh app solve through the runtime default pipeline ([77cbac1](https://github.com/SolverForge/solverforge-cli/commit/77cbac1cf52a389e19a849f4e9462066024faa9b))
* **scalar-group:** refuse solver configs whose construction reaches an assignment-owned target ([426260a](https://github.com/SolverForge/solverforge-cli/commit/426260a89c55d48c9581d71c07162ba4f3a49cc5))
* **server:** honor the PORT environment variable ([2e9487b](https://github.com/SolverForge/solverforge-cli/commit/2e9487b965486f3e892ad6b90a74356e2629d67a))
* **test:** pass arguments after -- to the test harness ([f6c9b31](https://github.com/SolverForge/solverforge-cli/commit/f6c9b316a239fc46b78f71d1912c824844fc2edb))


## [3.7.1](https://github.com/SolverForge/solverforge-cli/compare/v3.7.0...v3.7.1) (2026-10-27)


### Bug Fixes

* **mcp:** name ServerConfig instead of the deprecated ServerInfo alias ([b1ded48](https://github.com/SolverForge/solverforge-cli/commit/b1ded48fca966e0f7db5388df328d4b999eed1e5))
* **scaffold:** target rmcp 3.8.0 ([86127d7](https://github.com/SolverForge/solverforge-cli/commit/86127d70e7e778b7cb267212d0f94612a74c9c34))


## [3.7.0](https://github.com/SolverForge/solverforge-cli/compare/v3.6.0...v3.7.0) (2026-10-27)


### Features

* **config:** validate move_thread_count against the runtime's forms ([19ff0f8](https://github.com/SolverForge/solverforge-cli/commit/19ff0f85d474b8ebcc08db43f19bc50efc2a4fd8))
* **scaffold:** target SolverForge 0.23.0 ([207920d](https://github.com/SolverForge/solverforge-cli/commit/207920de6ee1d0f35715a13c320d0e71302ca588))


## [3.6.0](https://github.com/SolverForge/solverforge-cli/compare/v3.5.2...v3.6.0) (2026-10-18)


### Features

* **scaffold:** target SolverForge 0.22.0 ([12171c9](https://github.com/SolverForge/solverforge-cli/commit/12171c937564f6ff43d9c2ea3607f472b601ed49))


## [3.5.2](https://github.com/SolverForge/solverforge-cli/compare/v3.5.1...v3.5.2) (2026-10-15)


### Bug Fixes

* **scaffold:** target SolverForge 0.21.3 ([9c84b30](https://github.com/SolverForge/solverforge-cli/commit/9c84b300218be2fe60ca192cb5fa45b4b3321a6c))


## [3.5.1](https://github.com/SolverForge/solverforge-cli/compare/v3.5.0...v3.5.1) (2026-10-14)


### Bug Fixes

* **scaffold:** target SolverForge 0.21.2 ([26468d6](https://github.com/SolverForge/solverforge-cli/commit/26468d65d29ab69c747d5016803defc5f588610b))


## [3.5.0](https://github.com/SolverForge/solverforge-cli/compare/v3.4.0...v3.5.0) (2026-10-14)


### Features

* **scaffold:** target SolverForge 0.21.0 ([5052655](https://github.com/SolverForge/solverforge-cli/commit/505265593ed88e4028e2106a45ef808f577cde47))


## [3.4.0](https://github.com/SolverForge/solverforge-cli/compare/v3.3.3...v3.4.0) (2026-10-14)


### Features

* **scaffold:** target SolverForge 0.20.4 ([9a30041](https://github.com/SolverForge/solverforge-cli/commit/9a300412c906c484d3d46d23c10e6b5bcb1c824a))


## [3.3.3](https://github.com/SolverForge/solverforge-cli/compare/v3.3.2...v3.3.3) (2026-10-02)


### Bug Fixes

* **generate:** rewrite every score surface and resync demo data on destroy a615b59

## [3.3.2](https://github.com/SolverForge/solverforge-cli/compare/v3.3.1...v3.3.2) (2026-10-01)


### Bug Fixes

* **constraints:** stream generated skeletons from planning-solution accessors c94f9a7

## [3.3.1](https://github.com/SolverForge/solverforge-cli/compare/v3.3.0...v3.3.1) (2026-09-29)

### Bug Fixes

* **deps:** align the CLI dependency baseline with current releases 6355844

## [3.3.0](https://github.com/SolverForge/solverforge-cli/compare/v3.2.0...v3.3.0) (2026-09-29)

### Features

* **scaffold:** target rmcp 3.5.0 28ebfce

## [3.2.0](https://github.com/SolverForge/solverforge-cli/compare/v3.1.0...v3.2.0) (2026-09-29)

### Features

* **scaffold:** target SolverForge 0.19.7 9facc3b

## [3.1.0](https://github.com/SolverForge/solverforge-cli/compare/v3.0.0...v3.1.0) (2026-09-16)

### ⚠ BREAKING CHANGES

* **installer:** the skill installer keeps the agent-centric contract only.
  --only and the implicit ~/.agents/skills default stay removed; pass repeatable
  --agent opencode|claude|codex selectors (make install-skill ARGS='--agent
  <harness>'). Released as 3.1.0 because nothing after v3.0.0 was ever published.

### Features

* **cli:** target the latest released scaffold crates 41d642c
* **connect:** write MCP configs for opencode, Claude Code, and Cursor ad30780
* **skills:** consolidate skills and document the MCP and maps workflows 5fbe72c

### Bug Fixes

* **connect:** preserve JSONC and replace client configs atomically 1f5f862
* **installer:** enforce exact link ownership and the explicit layout contract 64df608
* **mcp:** make solve task results match the advertised output schema 89304a5
* **test:** make MCP connect assertions platform-portable ce9e9c0
* validate --skill names in the agent skill installer 7748859
* write skill ownership receipts at install time 1df9453

## [3.0.0](https://github.com/SolverForge/solverforge-cli/compare/v2.2.4...v3.0.0) (2026-09-15)


### Features

* add solverforge connect for MCP client setup e17a708
* add the --shell mcp scaffold b6e3e7f
* disclose the rmcp scaffold target in version output 779368e
* expose schema-optional DTOs for MCP tool contracts 6ae5ea4
* **skills:** add solverforge-modeling agent skill 48c923e
* **skills:** document the MCP output shell 5723023


### Bug Fixes

* **connect:** resolve Windows MCP executables 732681f
* **mcp:** preserve task and HTTP lifecycle integrity 9f251e5

## [2.2.4](///compare/v2.2.3...v2.2.4) (2026-08-14)

## [2.2.3](///compare/v2.2.2...v2.2.3) (2026-07-14)

### Features

* **scaffold:** target SolverForge 0.19.0 and solverforge-ui 0.7.0
* **modeling:** expose countable scalar ranges and the complete executable scalar/list metadata surface
* **modeling:** use list variables as the sole sequence and route model
* **scaffold:** execute seeded mixed scalar/list solves in runtime and browser release gates
* **telemetry:** expose full compact telemetry and separate bounded candidate-trace diagnostics
* **telemetry:** support qualified retained jobs with externally attested trace provenance

### Bug Fixes

* **config:** apply `.solverforgerc` server ports and validate candidate trace capacity
* **docs:** align repository contracts and runtime capability claims with the released targets

## [2.2.2](///compare/v2.2.0...v2.2.2) (2026-06-12)


### Bug Fixes

* **ci:** harden forgejo rust checks c033b2f

## [2.2.0](///compare/v2.0.4...v2.2.0) (2026-05-31)


### Features

* **cli:** surface current SolverForge modeling resources fe68122


### Bug Fixes

* **scaffold:** preserve absent non-web ui source e12cdef

## [2.0.4](///compare/v2.0.3...v2.0.4) (2026-05-05)

## [2.0.3](///compare/v2.0.2...v2.0.3) (2026-05-03)


### Bug Fixes

* **scaffold:** build analysis modal bodies safely e03c8e3
* **scaffold:** preserve zero-valued job ids 707cc72
* **scaffold:** retarget generated apps to latest upstream crates 4e8bac2

## [2.0.3](///compare/v2.0.2...v2.0.3) (2026-05-03)


### Bug Fixes

* **scaffold:** build analysis modal bodies safely e03c8e3
* **scaffold:** preserve zero-valued job ids 707cc72

## [2.0.2](///compare/v2.0.1...v2.0.2) (2026-05-03)


### Bug Fixes

* **scaffold:** align generated apps with solverforge 0.10.0 ff6cb45
* **scaffold:** retarget generated apps to solverforge-ui 0.6.4 86576f1

## [2.0.2](///compare/v2.0.1...v2.0.2) (2026-05-02)

### Bug Fixes

* retarget generated apps to solverforge 0.10.0
* align generated scalar variables with value_range_provider macro metadata
* borrow retained solver telemetry in generated lifecycle DTOs
* retarget generated apps to solverforge-ui 0.6.4

## [2.0.1](///compare/v2.0.0...v2.0.1) (2026-04-26)


### Bug Fixes

* normalize generated path diagnostics 01d2c3c

## [2.0.1](///compare/v2.0.0...v2.0.1) (2026-04-26)


### Bug Fixes

* normalize generated path diagnostics 01d2c3c
* align CLI and generated app dependency baselines with current releases
* align Rust version requirements to 1.95
* retarget generated apps to solverforge 0.9.1
* retarget generated apps to solverforge-ui 0.6.3

## [2.0.0](///compare/v1.1.3...v2.0.0) (2026-04-24)


### Features

* retarget generated apps to the scalar neutral scaffold feb37c3


### Bug Fixes

* retarget scaffold runtime to solverforge 0.8.8 2e1140d

## [1.1.3](///compare/v1.1.2...v1.1.3) (2026-04-13)


### Bug Fixes

* retarget scaffolds to solverforge 0.8.5 0a7a9d6

## [1.1.2](///compare/v1.1.1...v1.1.2) (2026-04-12)


### Bug Fixes

* **release:** align scaffold targets and add tag publishing 376ffa4
* **test:** resolve generated-app executables from cargo metadata b9d4cfa

## [1.1.2](///compare/v1.1.1...v1.1.2) (2026-04-12)


### Bug Fixes

* **release:** align scaffold targets and add tag publishing 376ffa4

## 1.1.1 (2026-04-12)


### Features

* add generated demo data scaffolding with safe legacy migration a2c1224
* align scaffolds with retained job lifecycle f4e9afc
* improve generated demo data and normalize planning ids 3b3b188
* unify the scaffold around a neutral shell and add phase-marked e2e pipelines 50f9880


### Bug Fixes

* add sequence board to list scaffold ad2938e
* align templates with solverforge-ui 0.3.0 db2da99
* **ci:** keep generated-app runtime checks in integration 357aafa
* generate entity emits correct Rust when adding a second planning variable 9d2d387
* harden config tests and Makefile workflows acfea6a
* make CI validate published scaffold targets b2b0950
* remove duplicate Debug/Clone derives from templates; revert solverforge version to 0.5.2 f3cf13f
* retarget scaffolds to solverforge 0.8.1 31f81da
* retarget scaffolds to solverforge 0.8.3 5597c33
* retarget scaffolds to solverforge-ui 0.4.2 1bd473a
* support stop and resume in scaffolded UI d3621e6
* **test:** make legacy loader scaffold setup newline-safe da3c3bb
* update scaffolds for solverforge-ui 0.3.1 a7fb472
* use solverforge ui cards in standard scaffold c0d9d9b
