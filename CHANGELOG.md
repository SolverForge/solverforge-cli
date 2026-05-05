# Changelog

All notable changes to this project will be documented in this file. See [commit-and-tag-version](https://github.com/absolute-version/commit-and-tag-version) for commit guidelines.

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
