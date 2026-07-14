use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_complete::Shell;

mod app_spec;
mod commands;
mod countable_range;
mod error;
mod list_variable_metadata;
mod managed_block;
mod model_contract;
mod model_id;
mod output;
mod rc;
mod scaffold_target;
mod scalar_variable_hooks;
mod solver_config;
mod template;
#[cfg(test)]
mod test_support;

use commands::model_resource::{ConflictRepairRequest, ScalarGroupRequest};
use commands::new::ScaffoldShell;
use error::CliResult;
use list_variable_metadata::ListVariableMetadata;
use scaffold_target::LONG_VERSION_TEXT;
use scalar_variable_hooks::ScalarVariableHooks;

const CLI_VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_SERVER_PORT: u16 = 7860;

fn parse_variable_kind(value: &str) -> Result<String, String> {
    match value {
        "scalar" => Ok("scalar".to_string()),
        "list" => Ok("list".to_string()),
        _ => Err("valid values: scalar, list".to_string()),
    }
}

const EXAMPLES: &str = "\x1b[1mExamples:\x1b[0m
  solverforge new my-optimizer
  solverforge new batch-optimizer --shell cli
  solverforge generate entity shift --planning-variable employee_idx
  solverforge generate constraint no_overlap --pair --hard
  solverforge server
  solverforge info
  solverforge check
  solverforge test
  solverforge routes
  solverforge config show";

#[derive(Parser)]
#[command(
    name = "solverforge",
    about = "CLI for scaffolding and managing SolverForge projects",
    version = CLI_VERSION,
    long_version = LONG_VERSION_TEXT,
    infer_subcommands = true,
    after_help = EXAMPLES,
)]
struct Cli {
    /// Suppress all output except errors
    #[arg(long, short, global = true)]
    quiet: bool,

    /// Show extra diagnostic output
    #[arg(long, short, global = true)]
    verbose: bool,

    /// Disable colored output (also respects NO_COLOR env var)
    #[arg(long, global = true)]
    no_color: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Scaffold a new SolverForge project
    #[command(
        after_help = "Examples:\n  solverforge new my-optimizer\n  solverforge new batch-optimizer --shell cli\n  solverforge new api-optimizer --shell api"
    )]
    New {
        /// Project name (directory that will be created)
        name: String,

        /// Generated shell: web, api, or cli
        #[arg(long, value_enum, default_value_t = ScaffoldShell::Web)]
        shell: ScaffoldShell,

        /// Skip running `git init` and initial commit
        #[arg(long)]
        skip_git: bool,

        /// Skip generating README.md
        #[arg(long)]
        skip_readme: bool,
    },
    /// Generate a new resource for the current project
    #[command(
        after_help = "Examples:\n  solverforge generate entity shift --planning-variable employee_idx\n  solverforge generate fact employee\n  solverforge generate variable stops --entity Route --kind list --elements visits\n  solverforge generate constraint no_overlap --pair --hard\n  solverforge generate solution schedule --score HardSoftScore\n  solverforge generate data\n  solverforge generate data --size large\n  solverforge generate data --mode stub"
    )]
    Generate {
        #[command(subcommand)]
        resource: Box<GenerateResource>,
    },
    /// Remove a resource from the current project
    #[command(
        after_help = "Examples:\n  solverforge destroy entity shift\n  solverforge destroy constraint no_overlap"
    )]
    Destroy {
        /// Skip confirmation prompt
        #[arg(long, short)]
        yes: bool,

        #[command(subcommand)]
        resource: DestroyResource,
    },
    /// Start the development server
    #[command(
        after_help = "Examples:\n  solverforge server\n  solverforge server --port 8080\n  solverforge server --debug"
    )]
    Server {
        /// Port to bind the server to; when omitted, use .solverforgerc and then 7860
        #[arg(long, short)]
        port: Option<u16>,

        /// Run in debug mode (faster compilation, slower runtime)
        #[arg(long)]
        debug: bool,
    },
    /// Show project summary: entities, facts, constraints, score type
    #[command(after_help = "Examples:\n  solverforge info")]
    Info,
    /// Validate project structure and configuration
    #[command(after_help = "Examples:\n  solverforge check")]
    Check,
    /// Run `cargo test` with optional passthrough arguments
    #[command(
        after_help = "Examples:\n  solverforge test\n  solverforge test -- --nocapture\n  solverforge test integration"
    )]
    Test {
        /// Extra arguments passed directly to `cargo test`
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        extra_args: Vec<String>,
    },
    /// List HTTP routes defined in src/api/
    #[command(after_help = "Examples:\n  solverforge routes")]
    Routes,
    /// Manage solver configuration (solver.toml)
    #[command(
        after_help = "Examples:\n  solverforge config show\n  solverforge config set termination.seconds_spent_limit 60"
    )]
    Config {
        #[command(subcommand)]
        subcommand: ConfigSubcommand,
    },
    /// Generate shell completions
    #[command(
        after_help = "Examples:\n  solverforge completions bash >> ~/.bashrc\n  solverforge completions zsh >> ~/.zshrc\n  solverforge completions fish > ~/.config/fish/completions/solverforge.fish"
    )]
    Completions {
        /// Shell to generate completions for
        shell: Shell,
    },
}

#[derive(Subcommand)]
enum ConfigSubcommand {
    /// Print the contents of solver.toml
    Show,
    /// Set a key in solver.toml (e.g. termination.seconds_spent_limit = 60)
    Set {
        /// Dotted key path (e.g. termination.seconds_spent_limit)
        key: String,
        /// New value
        value: String,
    },
}

#[derive(Subcommand)]
enum GenerateResource {
    /// Add a new constraint skeleton to src/constraints/
    #[command(
        after_help = "Examples:\n  solverforge generate constraint max_hours --unary --hard\n  solverforge generate constraint no_overlap --pair\n  solverforge generate constraint required_skill --join --hard"
    )]
    Constraint {
        /// Constraint module name in snake_case (e.g. max_hours)
        name: String,

        /// Hard constraint — must be satisfied (default)
        #[arg(long, conflicts_with = "soft")]
        hard: bool,

        /// Soft constraint — should be optimized
        #[arg(long, conflicts_with = "hard")]
        soft: bool,

        /// Penalize matching entities (for_each + filter + penalize)
        #[arg(long, conflicts_with_all = ["pair", "join", "balance", "reward", "runs", "presence", "collect_vec", "group_complement", "projected_group"])]
        unary: bool,

        /// Penalize conflicting pairs (for_each_unique_pair)
        #[arg(long, conflicts_with_all = ["unary", "join", "balance", "reward", "runs", "presence", "collect_vec", "group_complement", "projected_group"])]
        pair: bool,

        /// Penalize entity-fact mismatch (for_each + join)
        #[arg(long, conflicts_with_all = ["unary", "pair", "balance", "reward", "runs", "presence", "collect_vec", "group_complement", "projected_group"])]
        join: bool,

        /// Balance assignments across entities
        #[arg(long, conflicts_with_all = ["unary", "pair", "join", "reward", "runs", "presence", "collect_vec", "group_complement", "projected_group"])]
        balance: bool,

        /// Reward matching entities (for_each + filter + reward)
        #[arg(long, conflicts_with_all = ["unary", "pair", "join", "balance", "runs", "presence", "collect_vec", "group_complement", "projected_group"])]
        reward: bool,

        /// Generate a grouped consecutive-runs collector skeleton
        #[arg(long, conflicts_with_all = ["unary", "pair", "join", "balance", "reward", "presence", "collect_vec", "group_complement", "projected_group"])]
        runs: bool,

        /// Generate a grouped indexed-presence collector skeleton
        #[arg(long, conflicts_with_all = ["unary", "pair", "join", "balance", "reward", "runs", "collect_vec", "group_complement", "projected_group"])]
        presence: bool,

        /// Generate a grouped collect_vec collector skeleton
        #[arg(long, conflicts_with_all = ["unary", "pair", "join", "balance", "reward", "runs", "presence", "group_complement", "projected_group"])]
        collect_vec: bool,

        /// Generate a grouped complement skeleton
        #[arg(long, conflicts_with_all = ["unary", "pair", "join", "balance", "reward", "runs", "presence", "collect_vec", "projected_group"])]
        group_complement: bool,

        /// Generate a projected join plus grouped collector skeleton
        #[arg(long, conflicts_with_all = ["unary", "pair", "join", "balance", "reward", "runs", "presence", "collect_vec", "group_complement"])]
        projected_group: bool,

        /// Overwrite if constraint already exists
        #[arg(long, short)]
        force: bool,

        /// Preview changes without writing files
        #[arg(long)]
        pretend: bool,
    },
    /// Scaffold a planning entity struct in src/domain/
    #[command(
        after_help = "Examples:\n  solverforge generate entity shift --planning-variable employee_idx\n  solverforge generate entity task"
    )]
    Entity {
        /// Entity name in snake_case (e.g. shift)
        name: String,

        /// Planning variable field name (e.g. employee_idx)
        #[arg(long = "planning-variable", value_name = "FIELD")]
        planning_variable: Option<String>,

        /// Additional fields in "name:Type" format (repeatable, e.g. --field "start:String")
        #[arg(long = "field", value_name = "NAME:TYPE")]
        fields: Vec<String>,

        /// Overwrite if entity already exists
        #[arg(long, short)]
        force: bool,

        /// Preview changes without writing files
        #[arg(long)]
        pretend: bool,
    },
    /// Scaffold a problem fact struct in src/domain/
    #[command(
        after_help = "Examples:\n  solverforge generate fact employee\n  solverforge generate fact location --field \"lat:f64\" --field \"lng:f64\""
    )]
    Fact {
        /// Fact name in snake_case (e.g. employee)
        name: String,

        /// Additional fields in "name:Type" format (repeatable, e.g. --field "skill:String")
        #[arg(long = "field", value_name = "NAME:TYPE")]
        fields: Vec<String>,

        /// Overwrite if fact already exists
        #[arg(long, short)]
        force: bool,

        /// Preview changes without writing files
        #[arg(long)]
        pretend: bool,
    },
    /// Scaffold a planning solution struct in src/domain/
    #[command(
        after_help = "Examples:\n  solverforge generate solution schedule --score HardSoftScore"
    )]
    Solution {
        /// Solution name in snake_case (e.g. schedule)
        name: String,

        /// Score type (e.g. HardSoftScore, HardSoftDecimalScore)
        #[arg(long, value_name = "SCORE_TYPE", default_value = "HardSoftScore")]
        score: String,
    },
    /// Add a planning variable field to an existing entity
    #[command(
        after_help = "Examples:\n  solverforge generate variable employee_idx --entity Shift --kind scalar --range employees --allows-unassigned\n  solverforge generate variable hour --entity Shift --kind scalar --countable-range 0..24\n  solverforge generate variable employee_idx --entity Shift --kind scalar --range employees --candidate-values employee_candidates\n  solverforge generate variable stops --entity Route --kind list --elements visits"
    )]
    Variable(Box<VariableArgs>),
    /// Change the score type in the existing planning solution
    #[command(after_help = "Examples:\n  solverforge generate score HardSoftDecimalScore")]
    Score {
        /// Score type (e.g. HardSoftScore, HardSoftDecimalScore, HardMediumSoftScore, SoftScore, BendableScore<2, 3>)
        score_type: String,
    },
    /// Regenerate compiler-owned demo data from the project model
    #[command(
        after_help = "Examples:\n  solverforge generate data\n  solverforge generate data --size large\n  solverforge generate data --mode stub"
    )]
    Data {
        /// Data generation mode
        #[arg(long, value_parser = ["sample", "stub"], default_value = "sample")]
        mode: String,

        /// Default dataset size exposed by the generated demo data
        #[arg(long, value_parser = ["small", "standard", "large"])]
        size: Option<String>,
    },
    /// Declare a SolverForge scalar group for coupled scalar construction/search
    #[command(
        after_help = "Examples:\n  solverforge generate scalar-group required_assignment --assignment Task.resource_idx --required-entity required_task\n  solverforge generate scalar-group paired_assignment --candidates paired_candidates --target Task.primary_idx --target Task.secondary_idx"
    )]
    ScalarGroup(Box<ScalarGroupArgs>),
    /// Declare a conflict repair provider for a constraint
    #[command(
        after_help = "Examples:\n  solverforge generate conflict-repair required_assignment --provider repair_required_assignment"
    )]
    ConflictRepair(Box<ConflictRepairArgs>),
}

#[derive(Args)]
struct VariableArgs {
    /// Field name in snake_case (e.g. preferred_shift)
    field: String,

    /// Entity struct name (e.g. Shift)
    #[arg(long, value_name = "ENTITY_TYPE")]
    entity: String,

    /// Variable kind (`scalar` or `list`)
    #[arg(long, value_parser = parse_variable_kind)]
    kind: String,

    /// Scalar-variable value range collection (e.g. employees)
    #[arg(
        long,
        value_name = "FACT_COLLECTION",
        conflicts_with = "countable_range"
    )]
    range: Option<String>,

    /// Half-open scalar integer range (e.g. 0..24)
    #[arg(long, value_name = "FROM..TO", conflicts_with = "range")]
    countable_range: Option<String>,

    /// List-variable element collection (e.g. visits)
    #[arg(long, value_name = "FACT_COLLECTION")]
    elements: Option<String>,

    /// Stock list-domain profile; currently `cvrp`
    #[arg(long, value_parser = ["cvrp"], value_name = "PROFILE")]
    domain: Option<String>,

    /// Cross-entity list distance meter type/path
    #[arg(long, value_name = "RUST_PATH")]
    distance_meter: Option<String>,

    /// Within-entity list distance meter type/path
    #[arg(long, value_name = "RUST_PATH")]
    intra_distance_meter: Option<String>,

    /// Module providing route-local get/set/depot/distance/feasible hooks
    #[arg(long, value_name = "MODULE_PATH")]
    route_hooks: Option<String>,

    /// Module providing Clarke-Wright depot/distance/feasible hooks
    #[arg(long, value_name = "MODULE_PATH")]
    savings_hooks: Option<String>,

    /// Function assigning a savings metric class to each list owner
    #[arg(long, value_name = "FN_PATH")]
    savings_metric_class_fn: Option<String>,

    /// Function returning a fixed owner for each list element
    #[arg(long, value_name = "FN_PATH")]
    element_owner_fn: Option<String>,

    /// Function ranking list elements during construction
    #[arg(long, value_name = "FN_PATH")]
    construction_element_order_key: Option<String>,

    /// Function returning precedence duration for a list element
    #[arg(long, value_name = "FN_PATH")]
    precedence_duration_fn: Option<String>,

    /// Function appending precedence successors for a list element
    #[arg(long, value_name = "FN_PATH")]
    precedence_successors_fn: Option<String>,

    /// Additional solution trait required by the list metadata
    #[arg(long, value_name = "TRAIT_PATH")]
    solution_trait: Option<String>,

    /// Allow leaving the scalar variable unassigned
    #[arg(long, default_value_t = false)]
    allows_unassigned: bool,

    /// Scalar hook returning candidate value indexes for construction/search
    #[arg(long, value_name = "FN_PATH")]
    candidate_values: Option<String>,

    /// Scalar hook returning nearby value indexes for nearby change moves
    #[arg(long, value_name = "FN_PATH")]
    nearby_value_candidates: Option<String>,

    /// Scalar hook returning nearby entity indexes for nearby swap moves
    #[arg(long, value_name = "FN_PATH")]
    nearby_entity_candidates: Option<String>,

    /// Scalar distance meter for nearby value candidates
    #[arg(long, value_name = "FN_PATH")]
    nearby_value_distance_meter: Option<String>,

    /// Scalar distance meter for nearby entity candidates
    #[arg(long, value_name = "FN_PATH")]
    nearby_entity_distance_meter: Option<String>,

    /// Scalar construction hook ranking entity assignment order
    #[arg(long, value_name = "FN_PATH")]
    construction_entity_order_key: Option<String>,

    /// Scalar construction hook ranking candidate value order
    #[arg(long, value_name = "FN_PATH")]
    construction_value_order_key: Option<String>,
}

#[derive(Args)]
struct ScalarGroupArgs {
    /// Group name used by ScalarGroup and solver.toml group_name
    name: String,

    /// Assignment-backed scalar target in Entity.field form
    #[arg(long, value_name = "ENTITY.FIELD", conflicts_with = "candidates")]
    assignment: Option<String>,

    /// Candidate-backed provider function path
    #[arg(long, value_name = "FN_PATH", conflicts_with = "assignment")]
    candidates: Option<String>,

    /// Candidate-backed scalar target in Entity.field form; repeat for multi-target groups
    #[arg(long = "target", value_name = "ENTITY.FIELD")]
    targets: Vec<String>,

    /// Assignment hook deciding whether an entity must be assigned
    #[arg(long, value_name = "FN_PATH")]
    required_entity: Option<String>,

    /// Assignment hook returning a capacity bucket for entity/value pairs
    #[arg(long, value_name = "FN_PATH")]
    capacity_key: Option<String>,

    /// Assignment hook filtering legal pairwise assignments
    #[arg(long, value_name = "FN_PATH")]
    assignment_rule: Option<String>,

    /// Assignment hook returning sequence position for an entity
    #[arg(long, value_name = "FN_PATH")]
    position_key: Option<String>,

    /// Assignment hook returning sequence key for entity/value pairs
    #[arg(long, value_name = "FN_PATH")]
    sequence_key: Option<String>,

    /// Assignment hook ranking entity construction order
    #[arg(long, value_name = "FN_PATH")]
    entity_order: Option<String>,

    /// Assignment hook ranking candidate values
    #[arg(long, value_name = "FN_PATH")]
    value_order: Option<String>,

    /// Limit candidate values per scalar target
    #[arg(long, value_name = "N")]
    value_candidate_limit: Option<usize>,

    /// Limit grouped construction candidates
    #[arg(long, value_name = "N")]
    group_candidate_limit: Option<usize>,

    /// Limit grouped scalar local-search moves per step
    #[arg(long, value_name = "N")]
    max_moves_per_step: Option<usize>,

    /// Limit augmenting-path depth for assignment groups
    #[arg(long, value_name = "N")]
    max_augmenting_depth: Option<usize>,

    /// Limit rematch size for assignment groups
    #[arg(long, value_name = "N")]
    max_rematch_size: Option<usize>,

    /// Do not add matching solver.toml phases
    #[arg(long)]
    skip_solver_config: bool,
}

#[derive(Args)]
struct ConflictRepairArgs {
    /// Exact constraint ID matched by the repair selector
    constraint: String,

    /// Repair provider function path
    #[arg(long, value_name = "FN_PATH")]
    provider: String,

    /// Move selector kind to add to solver.toml
    #[arg(long, value_parser = ["compound", "conflict"], default_value = "compound")]
    selector: String,

    /// Limit conflict matches per step
    #[arg(long, value_name = "N")]
    max_matches_per_step: Option<usize>,

    /// Limit repairs per matched conflict
    #[arg(long, value_name = "N")]
    max_repairs_per_match: Option<usize>,

    /// Limit emitted repair moves per step
    #[arg(long, value_name = "N")]
    max_moves_per_step: Option<usize>,

    /// Allow selectors to match soft-constraint conflicts
    #[arg(long)]
    include_soft_matches: bool,

    /// Do not add a matching solver.toml phase
    #[arg(long)]
    skip_solver_config: bool,
}

#[derive(Subcommand)]
enum DestroyResource {
    /// Remove the planning solution struct
    Solution,
    /// Remove a planning entity struct
    Entity {
        /// Entity name to remove
        name: String,
    },
    /// Remove a planning variable field from an entity
    Variable {
        /// Variable field to remove
        field: String,

        /// Entity struct name (e.g. Shift)
        #[arg(long, value_name = "ENTITY_TYPE")]
        entity: String,
    },
    /// Remove a problem fact struct
    Fact {
        /// Fact name to remove
        name: String,
    },
    /// Remove a constraint
    Constraint {
        /// Exact constraint ID to remove
        name: String,
    },
    /// Remove a scalar group declaration
    ScalarGroup {
        /// Scalar group name to remove
        name: String,
    },
    /// Remove a conflict repair declaration
    ConflictRepair {
        /// Exact constraint ID to remove
        name: String,
    },
}

fn main() {
    // Load rc config first so CLI flags can override it.
    let rc = rc::load_rc().unwrap_or_default();

    let cli = Cli::parse();

    // Configure output — CLI flags override rc config.
    let quiet = cli.quiet || rc.quiet;
    if quiet {
        output::set_verbosity(0);
    } else if cli.verbose {
        output::set_verbosity(2);
    }
    if cli.no_color || rc.no_color || std::env::var("NO_COLOR").is_ok() {
        output::set_no_color(true);
    }

    let result: CliResult = match cli.command {
        Command::New {
            name,
            shell,
            skip_git,
            skip_readme,
        } => commands::new::run(&name, shell, skip_git, skip_readme, quiet),
        Command::Generate { resource } => match *resource {
            GenerateResource::Constraint {
                name,
                hard: _,
                soft,
                unary,
                pair,
                join,
                balance,
                reward,
                runs,
                presence,
                collect_vec,
                group_complement,
                projected_group,
                force,
                pretend,
            } => commands::generate_constraint::run(
                &name,
                soft,
                unary,
                pair,
                join,
                balance,
                reward,
                runs,
                presence,
                collect_vec,
                group_complement,
                projected_group,
                force,
                pretend,
            ),
            GenerateResource::Entity {
                name,
                planning_variable,
                fields,
                force,
                pretend,
            } => commands::generate_domain::run_entity(
                &name,
                planning_variable.as_deref(),
                &fields,
                force,
                pretend,
            ),
            GenerateResource::Fact {
                name,
                fields,
                force,
                pretend,
            } => commands::generate_domain::run_fact(&name, &fields, force, pretend),
            GenerateResource::Solution { name, score } => {
                commands::generate_domain::run_solution(&name, &score)
            }
            GenerateResource::Variable(variable_args) => {
                let VariableArgs {
                    field,
                    entity,
                    kind,
                    range,
                    countable_range,
                    elements,
                    domain,
                    distance_meter,
                    intra_distance_meter,
                    route_hooks,
                    savings_hooks,
                    savings_metric_class_fn,
                    element_owner_fn,
                    construction_element_order_key,
                    precedence_duration_fn,
                    precedence_successors_fn,
                    solution_trait,
                    allows_unassigned,
                    candidate_values,
                    nearby_value_candidates,
                    nearby_entity_candidates,
                    nearby_value_distance_meter,
                    nearby_entity_distance_meter,
                    construction_entity_order_key,
                    construction_value_order_key,
                } = *variable_args;
                commands::generate_domain::run_variable(
                    commands::generate_domain::VariableRequest {
                        field,
                        entity,
                        kind,
                        range,
                        countable_range,
                        elements,
                        allows_unassigned,
                        scalar_hooks: ScalarVariableHooks {
                            candidate_values,
                            nearby_value_candidates,
                            nearby_entity_candidates,
                            nearby_value_distance_meter,
                            nearby_entity_distance_meter,
                            construction_entity_order_key,
                            construction_value_order_key,
                        },
                        list_metadata: ListVariableMetadata {
                            domain,
                            distance_meter,
                            intra_distance_meter,
                            route_hooks,
                            savings_hooks,
                            savings_metric_class_fn,
                            element_owner_fn,
                            construction_element_order_key,
                            precedence_duration_fn,
                            precedence_successors_fn,
                            solution_trait,
                        },
                    },
                )
            }
            GenerateResource::Score { score_type } => {
                commands::generate_domain::run_score(&score_type)
            }
            GenerateResource::Data { mode, size } => {
                commands::generate_domain::run_data(&mode, size.as_deref())
            }
            GenerateResource::ScalarGroup(args) => {
                let ScalarGroupArgs {
                    name,
                    assignment,
                    candidates,
                    targets,
                    required_entity,
                    capacity_key,
                    assignment_rule,
                    position_key,
                    sequence_key,
                    entity_order,
                    value_order,
                    value_candidate_limit,
                    group_candidate_limit,
                    max_moves_per_step,
                    max_augmenting_depth,
                    max_rematch_size,
                    skip_solver_config,
                } = *args;
                commands::model_resource::run_scalar_group(ScalarGroupRequest {
                    name,
                    assignment,
                    candidates,
                    targets,
                    required_entity,
                    capacity_key,
                    assignment_rule,
                    position_key,
                    sequence_key,
                    entity_order,
                    value_order,
                    value_candidate_limit,
                    group_candidate_limit,
                    max_moves_per_step,
                    max_augmenting_depth,
                    max_rematch_size,
                    skip_solver_config,
                })
            }
            GenerateResource::ConflictRepair(args) => {
                let ConflictRepairArgs {
                    constraint,
                    provider,
                    selector,
                    max_matches_per_step,
                    max_repairs_per_match,
                    max_moves_per_step,
                    include_soft_matches,
                    skip_solver_config,
                } = *args;
                commands::model_resource::run_conflict_repair(ConflictRepairRequest {
                    constraint,
                    provider,
                    selector,
                    max_matches_per_step,
                    max_repairs_per_match,
                    max_moves_per_step,
                    include_soft_matches,
                    skip_solver_config,
                })
            }
        },
        Command::Destroy {
            yes,
            resource: DestroyResource::Solution,
        } => commands::destroy::run_solution(yes),
        Command::Destroy {
            yes,
            resource: DestroyResource::Entity { name },
        } => commands::destroy::run_entity(&name, yes),
        Command::Destroy {
            yes,
            resource: DestroyResource::Variable { field, entity },
        } => commands::destroy::run_variable(&field, &entity, yes),
        Command::Destroy {
            yes,
            resource: DestroyResource::Fact { name },
        } => commands::destroy::run_fact(&name, yes),
        Command::Destroy {
            yes,
            resource: DestroyResource::Constraint { name },
        } => commands::destroy::run_constraint(&name, yes),
        Command::Destroy {
            yes,
            resource: DestroyResource::ScalarGroup { name },
        } => commands::destroy::run_scalar_group(&name, yes),
        Command::Destroy {
            yes,
            resource: DestroyResource::ConflictRepair { name },
        } => commands::destroy::run_conflict_repair(&name, yes),
        Command::Server { port, debug } => {
            commands::server::run(resolve_server_port(port, rc.port), debug)
        }
        Command::Info => commands::info::run(),
        Command::Check => commands::check::run(),
        Command::Test { extra_args } => commands::test::run(&extra_args),
        Command::Routes => commands::routes::run(),
        Command::Config {
            subcommand: ConfigSubcommand::Show,
        } => commands::config::run_show(),
        Command::Config {
            subcommand: ConfigSubcommand::Set { key, value },
        } => commands::config::run_set(&key, &value),
        Command::Completions { shell } => {
            let mut cmd = Cli::command();
            clap_complete::generate(shell, &mut cmd, "solverforge", &mut std::io::stdout());
            Ok(())
        }
    };

    if let Err(e) = result {
        output::print_error(&e.to_string());
        std::process::exit(1);
    }
}

fn resolve_server_port(cli_port: Option<u16>, rc_port: Option<u16>) -> u16 {
    cli_port.or(rc_port).unwrap_or(DEFAULT_SERVER_PORT)
}

#[cfg(test)]
mod main_tests {
    use super::{resolve_server_port, Cli, DEFAULT_SERVER_PORT};
    use clap::{error::ErrorKind, Parser};

    #[test]
    fn explicit_server_port_overrides_rc_port() {
        assert_eq!(resolve_server_port(Some(9000), Some(8080)), 9000);
    }

    #[test]
    fn rc_server_port_overrides_builtin_default() {
        assert_eq!(resolve_server_port(None, Some(8080)), 8080);
    }

    #[test]
    fn server_port_uses_builtin_default_without_preference() {
        assert_eq!(resolve_server_port(None, None), DEFAULT_SERVER_PORT);
    }

    #[test]
    fn variable_command_rejects_scalar_predecessor_flags() {
        for flag in ["--chained", "--inverse-shadow", "--anchor-shadow"] {
            let error = Cli::try_parse_from([
                "solverforge",
                "generate",
                "variable",
                "previous",
                "--entity",
                "Visit",
                "--kind",
                "scalar",
                "--range",
                "depots",
                flag,
            ])
            .err()
            .expect("scalar predecessor flags must not be part of the public CLI");
            assert_eq!(error.kind(), ErrorKind::UnknownArgument, "flag: {flag}");
        }
    }
}
