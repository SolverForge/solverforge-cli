# Changelog

All notable changes to this project will be documented in this file. See [commit-and-tag-version](https://github.com/absolute-version/commit-and-tag-version) for commit guidelines.

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
