/* {{project_name}} — neutral constraint optimizer built with SolverForge

Structure:
  domain/      — Plan (solution) plus CLI-generated entities and facts
  constraints/ — Scoring rules
  solver/      — Engine, service, termination config
  api/         — HTTP API (axum)
  data/        — Demo data / data loading */

pub mod api;
pub mod constraints;
pub mod data;
pub mod domain;
pub mod solver;
