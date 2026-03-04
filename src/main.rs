use clap::{Parser, Subcommand};

mod commands;
mod template;

#[derive(Parser)]
#[command(
    name = "solverforge",
    about = "CLI for SolverForge — a zero-erasure constraint solver in Rust",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Scaffold a new SolverForge project
    ///
    /// Variable class (required, mutually exclusive):
    ///
    ///   --basic     Standard variable — each entity holds one assigned value
    ///   --list      List variable     — each entity owns an ordered sequence
    ///
    /// Specializations (append after the flag with /):
    ///
    ///   --basic/employee-scheduling
    New {
        /// Project name (directory that will be created)
        name: String,

        /// Scaffold a standard-variable project (optionally: --basic/employee-scheduling)
        #[arg(long = "basic", value_name = "SPECIALIZATION", num_args = 0..=1, require_equals = true)]
        basic: Option<Option<String>>,

        /// Scaffold a list-variable project
        #[arg(long = "list", value_name = "SPECIALIZATION", num_args = 0..=1, require_equals = true)]
        list: Option<Option<String>>,
    },
    /// Add a new constraint to the current project
    Add {
        #[command(subcommand)]
        resource: AddResource,
    },
    /// Start the development server (wraps `cargo run --release`)
    Server,
}

#[derive(Subcommand)]
enum AddResource {
    /// Add a new constraint skeleton to src/constraints/
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
        #[arg(long, conflicts_with_all = ["pair", "join", "balance", "reward"])]
        unary: bool,

        /// Penalize conflicting pairs (for_each_unique_pair)
        #[arg(long, conflicts_with_all = ["unary", "join", "balance", "reward"])]
        pair: bool,

        /// Penalize entity-fact mismatch (for_each + join)
        #[arg(long, conflicts_with_all = ["unary", "pair", "balance", "reward"])]
        join: bool,

        /// Balance assignments across entities
        #[arg(long, conflicts_with_all = ["unary", "pair", "join", "reward"])]
        balance: bool,

        /// Reward matching entities (for_each + filter + reward)
        #[arg(long, conflicts_with_all = ["unary", "pair", "join", "balance"])]
        reward: bool,
    },
    /// Scaffold a planning entity struct in src/domain/
    Entity {
        /// Entity name in snake_case (e.g. shift)
        name: String,

        /// Planning variable field name (e.g. employee_idx)
        #[arg(long = "planning-variable", value_name = "FIELD")]
        planning_variable: Option<String>,
    },
    /// Scaffold a problem fact struct in src/domain/
    Fact {
        /// Fact name in snake_case (e.g. employee)
        name: String,
    },
    /// Scaffold a planning solution struct in src/domain/
    Solution {
        /// Solution name in snake_case (e.g. schedule)
        name: String,

        /// Score type (e.g. HardSoftScore, HardSoftDecimalScore)
        #[arg(long, value_name = "SCORE_TYPE", default_value = "HardSoftScore")]
        score: String,
    },
    /// Add a planning variable field to an existing entity
    Variable {
        /// Field name in snake_case (e.g. preferred_shift)
        field: String,

        /// Entity struct name (e.g. Shift)
        #[arg(long, value_name = "ENTITY_TYPE")]
        entity: String,
    },
    /// Change the score type in the existing planning solution
    Score {
        /// Score type (e.g. HardSoftScore, HardSoftDecimalScore, HardMediumSoftScore, SimpleScore)
        score_type: String,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::New { name, basic, list } => {
            let is_basic = basic.is_some();
            let is_list = list.is_some();
            let specialization: Option<String> = basic.flatten().or_else(|| list.flatten());

            match commands::new::Template::parse(is_basic, is_list, specialization.as_deref()) {
                Ok(template) => commands::new::run(&name, template),
                Err(e) => Err(e),
            }
        }
        Command::Add {
            resource: AddResource::Constraint { name, hard: _, soft, unary, pair, join, balance, reward },
        } => commands::add_constraint::run(&name, soft, unary, pair, join, balance, reward),
        Command::Add { resource: AddResource::Entity { name, planning_variable } } => {
            commands::add_domain::run_entity(&name, planning_variable.as_deref())
        }
        Command::Add { resource: AddResource::Fact { name } } => {
            commands::add_domain::run_fact(&name)
        }
        Command::Add { resource: AddResource::Solution { name, score } } => {
            commands::add_domain::run_solution(&name, &score)
        }
        Command::Add { resource: AddResource::Variable { field, entity } } => {
            commands::add_domain::run_variable(&field, &entity)
        }
        Command::Add { resource: AddResource::Score { score_type } } => {
            commands::add_domain::run_score(&score_type)
        }
        Command::Server => commands::server::run(),
    };

    if let Err(e) = result {
        eprintln!("{}: {}", owo_colors::OwoColorize::bright_red(&"error"), e);
        std::process::exit(1);
    }
}
