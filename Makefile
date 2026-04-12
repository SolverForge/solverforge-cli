# SolverForge CLI Makefile v1.1.2
# CLI-focused build and validation workflow

# ============== Colors & Symbols ==============
GREEN := \033[92m
CYAN := \033[96m
YELLOW := \033[93m
MAGENTA := \033[95m
RED := \033[91m
GRAY := \033[90m
EMERALD := \033[38;2;16;185;129m
BOLD := \033[1m
RESET := \033[0m

CHECK := OK
CROSS := XX
ARROW := >
PROGRESS := ->

# ============== Project Metadata ==============
VERSION := $(shell grep -m1 '^version' Cargo.toml | sed 's/version = "\(.*\)"/\1/')
BIN := solverforge
RUST_VERSION := 1.80+

# ============== Phony Targets ==============
.PHONY: banner help build build-release run install test test-quick test-scaffold test-runtime \
        test-e2e install-e2e test-full test-ignored test-unit test-one lint fmt fmt-check clippy \
        pre-commit ci-local pre-release publish-dry-run clean version

# ============== Default Target ==============
.DEFAULT_GOAL := help

# ============== Banner ==============
banner:
	@printf -- "$(EMERALD)$(BOLD)  ____        _                _____\n"
	@printf -- " / ___|  ___ | |_   _____ _ __|  ___|__  _ __ __ _  ___\n"
	@printf -- " \\___ \\\\ / _ \\\\| \\\\ \\\\ / / _ \\\\ '__| |_ / _ \\\\| '__/ _\` |/ _ \\\\\n"
	@printf -- "  ___) | (_) | |\\\\ V /  __/ |  |  _| (_) | | | (_| |  __/\n"
	@printf -- " |____/ \\\\___/|_| \\_/ \\___|_|  |_|  \\___/|_|  \\__, |\\___|\n"
	@printf -- "                                             |___/$(RESET)\n"
	@printf -- "  $(GRAY)v$(VERSION)$(RESET) $(EMERALD)SolverForge CLI Build System$(RESET)\n\n"

# ============== Build Targets ==============
build: banner
	@printf -- "$(CYAN)$(BOLD)==== Build Debug =====================================$(RESET)\n\n"
	@printf -- "$(ARROW) Building $(BIN)...\n"
	@cargo build && \
		printf -- "$(GREEN)$(CHECK) Build successful$(RESET)\n\n" || \
		(printf -- "$(RED)$(CROSS) Build failed$(RESET)\n\n" && exit 1)

build-release: banner
	@printf -- "$(CYAN)$(BOLD)==== Build Release ===================================$(RESET)\n\n"
	@printf -- "$(ARROW) Building $(BIN) in release mode...\n"
	@cargo build --release && \
		printf -- "$(GREEN)$(CHECK) Release build successful$(RESET)\n\n" || \
		(printf -- "$(RED)$(CROSS) Release build failed$(RESET)\n\n" && exit 1)

run:
	@cargo run -- $(ARGS)

install:
	@printf -- "$(PROGRESS) Installing $(BIN) from the current checkout...\n"
	@cargo install --path . && \
		printf -- "$(GREEN)$(CHECK) Installed$(RESET)\n" || \
		(printf -- "$(RED)$(CROSS) Install failed$(RESET)\n" && exit 1)

# ============== Test Targets ==============
test: banner
	@printf -- "$(CYAN)$(BOLD)==== Rust Test Suite =================================$(RESET)\n\n"
	@printf -- "$(ARROW) Running cargo test...\n"
	@cargo test && \
		printf -- "\n$(GREEN)$(CHECK) Rust tests passed$(RESET)\n\n" || \
		(printf -- "\n$(RED)$(CROSS) Rust tests failed$(RESET)\n\n" && exit 1)

test-quick:
	@printf -- "$(PROGRESS) Running quick binary/unit coverage...\n"
	@cargo test --bin $(BIN) --quiet && \
		printf -- "$(GREEN)$(CHECK) Quick tests passed$(RESET)\n" || \
		(printf -- "$(RED)$(CROSS) Quick tests failed$(RESET)\n" && exit 1)

test-scaffold:
	@printf -- "$(PROGRESS) Running scaffold contract tests...\n"
	@cargo test --test scaffold_test -- --nocapture --test-threads=1 && \
		printf -- "$(GREEN)$(CHECK) Scaffold tests passed$(RESET)\n" || \
		(printf -- "$(RED)$(CROSS) Scaffold tests failed$(RESET)\n" && exit 1)

test-runtime: banner
	@printf -- "$(CYAN)$(BOLD)==== Runtime Pipeline ================================$(RESET)\n\n"
	@printf -- "$(ARROW) Running generated-app runtime pipeline tests...\n"
	@cargo test --test runtime_pipeline_test -- --nocapture --test-threads=1 && \
		printf -- "\n$(GREEN)$(CHECK) Runtime pipeline tests passed$(RESET)\n\n" || \
		(printf -- "\n$(RED)$(CROSS) Runtime pipeline tests failed$(RESET)\n\n" && exit 1)

install-e2e:
	@printf -- "$(PROGRESS) Installing Playwright Chromium...\n"
	@npm run install:e2e && \
		printf -- "$(GREEN)$(CHECK) Playwright Chromium installed$(RESET)\n" || \
		(printf -- "$(RED)$(CROSS) Playwright install failed$(RESET)\n" && exit 1)

test-e2e: banner
	@printf -- "$(CYAN)$(BOLD)==== Browser Pipeline ================================$(RESET)\n\n"
	@printf -- "$(ARROW) Running Playwright browser tests...\n"
	@npm run test:e2e && \
		printf -- "\n$(GREEN)$(CHECK) Playwright tests passed$(RESET)\n\n" || \
		(printf -- "\n$(RED)$(CROSS) Playwright tests failed$(RESET)\n\n" && exit 1)

test-full: banner
	@printf -- "$(CYAN)$(BOLD)==== Full Validation =================================$(RESET)\n\n"
	@printf -- "$(PROGRESS) Phase 1/4: Rust unit and binary tests...\n"
	@cargo test --bin $(BIN)
	@printf -- "$(PROGRESS) Phase 2/4: scaffold contract tests...\n"
	@cargo test --test scaffold_test -- --nocapture --test-threads=1
	@printf -- "$(PROGRESS) Phase 3/4: runtime pipeline tests...\n"
	@cargo test --test runtime_pipeline_test -- --nocapture --test-threads=1
	@printf -- "$(PROGRESS) Phase 4/4: Playwright browser tests...\n"
	@npm run test:e2e
	@printf -- "\n$(GREEN)$(CHECK) Full end-to-end validation passed$(RESET)\n\n"

test-ignored:
	@printf -- "$(PROGRESS) Running ignored scaffold tests...\n"
	@cargo test -- --ignored && \
		printf -- "$(GREEN)$(CHECK) Ignored tests passed$(RESET)\n" || \
		(printf -- "$(RED)$(CROSS) Ignored tests failed$(RESET)\n" && exit 1)

test-unit:
	@printf -- "$(PROGRESS) Running unit tests in $(BIN)...\n"
	@cargo test --bin $(BIN) && \
		printf -- "$(GREEN)$(CHECK) Unit tests passed$(RESET)\n" || \
		(printf -- "$(RED)$(CROSS) Unit tests failed$(RESET)\n" && exit 1)

test-one:
	@printf -- "$(PROGRESS) Running test: $(YELLOW)$(TEST)$(RESET)\n"
	@cargo test $(TEST) -- --nocapture

# ============== Lint & Format ==============
lint: banner fmt-check clippy
	@printf -- "\n$(GREEN)$(BOLD)$(CHECK) Lint checks passed$(RESET)\n\n"

fmt:
	@printf -- "$(PROGRESS) Formatting code...\n"
	@cargo fmt --all
	@printf -- "$(GREEN)$(CHECK) Code formatted$(RESET)\n"

fmt-check:
	@printf -- "$(PROGRESS) Checking formatting...\n"
	@cargo fmt --all -- --check && \
		printf -- "$(GREEN)$(CHECK) Formatting valid$(RESET)\n" || \
		(printf -- "$(RED)$(CROSS) Formatting issues found$(RESET)\n" && exit 1)

clippy:
	@printf -- "$(PROGRESS) Running clippy...\n"
	@cargo clippy --all-targets -- -D warnings && \
		printf -- "$(GREEN)$(CHECK) Clippy passed$(RESET)\n" || \
		(printf -- "$(RED)$(CROSS) Clippy warnings found$(RESET)\n" && exit 1)

pre-commit:
	@printf -- "$(PROGRESS) Running pre-commit hooks...\n"
	@pre-commit run --all-files && \
		printf -- "$(GREEN)$(CHECK) Pre-commit hooks passed$(RESET)\n" || \
		(printf -- "$(RED)$(CROSS) Pre-commit hooks failed$(RESET)\n" && exit 1)

# ============== CI Simulation ==============
ci-local: banner
	@printf -- "$(CYAN)$(BOLD)==== Local CI Simulation ============================$(RESET)\n\n"
	@printf -- "$(ARROW) Simulating the main GitHub Actions validation flow...\n\n"
	@printf -- "$(PROGRESS) Step 1/5: format check...\n"
	@$(MAKE) fmt-check --no-print-directory
	@printf -- "$(PROGRESS) Step 2/5: build...\n"
	@cargo build --quiet && printf -- "$(GREEN)$(CHECK) Build passed$(RESET)\n"
	@printf -- "$(PROGRESS) Step 3/5: clippy...\n"
	@$(MAKE) clippy --no-print-directory
	@printf -- "$(PROGRESS) Step 4/5: scaffold contract tests...\n"
	@cargo test --test scaffold_test -- --nocapture --test-threads=1 && \
		printf -- "$(GREEN)$(CHECK) Scaffold tests passed$(RESET)\n"
	@printf -- "$(PROGRESS) Step 5/5: runtime pipeline tests...\n"
	@cargo test --test runtime_pipeline_test -- --nocapture --test-threads=1 && \
		printf -- "$(GREEN)$(CHECK) Runtime tests passed$(RESET)\n"
	@printf -- "\n$(GREEN)$(BOLD)$(CHECK) Local CI simulation passed$(RESET)\n\n"

# ============== Release Validation ==============
pre-release: banner
	@printf -- "$(CYAN)$(BOLD)==== Pre-Release Validation ========================$(RESET)\n\n"
	@$(MAKE) fmt-check --no-print-directory
	@$(MAKE) clippy --no-print-directory
	@printf -- "$(PROGRESS) Running scaffold contract tests...\n"
	@cargo test --test scaffold_test -- --nocapture --test-threads=1 && \
		printf -- "$(GREEN)$(CHECK) Scaffold tests passed$(RESET)\n"
	@printf -- "$(PROGRESS) Running runtime pipeline tests...\n"
	@cargo test --test runtime_pipeline_test -- --nocapture --test-threads=1 && \
		printf -- "$(GREEN)$(CHECK) Runtime pipeline tests passed$(RESET)\n"
	@printf -- "$(PROGRESS) Running Playwright browser tests...\n"
	@npm run test:e2e && \
		printf -- "$(GREEN)$(CHECK) Playwright browser tests passed$(RESET)\n"
	@printf -- "$(PROGRESS) Running cargo publish --dry-run...\n"
	@cargo publish --dry-run --allow-dirty --quiet && \
		printf -- "$(GREEN)$(CHECK) Publish dry-run passed$(RESET)\n"
	@printf -- "\n$(GREEN)$(BOLD)$(CHECK) Ready for release v$(VERSION)$(RESET)\n\n"

publish-dry-run:
	@printf -- "$(PROGRESS) Running cargo publish --dry-run...\n"
	@cargo publish --dry-run --allow-dirty --quiet && \
		printf -- "$(GREEN)$(CHECK) Publish dry-run passed$(RESET)\n" || \
		(printf -- "$(RED)$(CROSS) Publish dry-run failed$(RESET)\n" && exit 1)

# ============== Utilities ==============
clean:
	@printf -- "$(ARROW) Cleaning build artifacts...\n"
	@cargo clean
	@printf -- "$(GREEN)$(CHECK) Clean complete$(RESET)\n"

version:
	@printf -- "$(CYAN)Current version:$(RESET) $(YELLOW)$(BOLD)$(VERSION)$(RESET)\n"

help: banner
	@printf -- "$(BOLD)Build$(RESET)\n"
	@printf -- "  $(GREEN)make build$(RESET)            Build the CLI in debug mode\n"
	@printf -- "  $(GREEN)make build-release$(RESET)    Build the CLI in release mode\n"
	@printf -- "  $(GREEN)make run ARGS=\"...\"$(RESET) Run the CLI locally\n"
	@printf -- "  $(GREEN)make install$(RESET)          Install the CLI with cargo install --path .\n\n"
	@printf -- "$(BOLD)Test$(RESET)\n"
	@printf -- "  $(GREEN)make test$(RESET)             Run the full Rust test suite\n"
	@printf -- "  $(GREEN)make test-quick$(RESET)       Run quick binary/unit coverage\n"
	@printf -- "  $(GREEN)make test-scaffold$(RESET)    Run scaffold contract tests\n"
	@printf -- "  $(GREEN)make test-runtime$(RESET)     Run generated-app runtime pipeline tests\n"
	@printf -- "  $(GREEN)make install-e2e$(RESET)      Install Playwright Chromium\n"
	@printf -- "  $(GREEN)make test-e2e$(RESET)         Run Playwright browser tests\n"
	@printf -- "  $(GREEN)make test-full$(RESET)        Run scaffold, runtime, and browser validation\n"
	@printf -- "  $(GREEN)make test-ignored$(RESET)     Run ignored tests\n"
	@printf -- "  $(GREEN)make test-unit$(RESET)        Run unit tests in the binary target\n"
	@printf -- "  $(GREEN)make test-one TEST=...$(RESET) Run one selected test\n\n"
	@printf -- "$(BOLD)Quality$(RESET)\n"
	@printf -- "  $(GREEN)make fmt$(RESET)              Format Rust code\n"
	@printf -- "  $(GREEN)make fmt-check$(RESET)        Check formatting\n"
	@printf -- "  $(GREEN)make clippy$(RESET)           Run clippy with warnings denied\n"
	@printf -- "  $(GREEN)make lint$(RESET)             Run fmt-check and clippy\n"
	@printf -- "  $(GREEN)make pre-commit$(RESET)       Run repository hooks\n"
	@printf -- "  $(GREEN)make ci-local$(RESET)         Simulate the primary CI flow locally\n\n"
	@printf -- "$(BOLD)Release$(RESET)\n"
	@printf -- "  $(GREEN)make pre-release$(RESET)      Run release validation checks\n"
	@printf -- "  $(GREEN)make publish-dry-run$(RESET)  Run cargo publish --dry-run\n"
	@printf -- "  $(GREEN)make version$(RESET)          Print the current package version\n"
	@printf -- "  $(GREEN)make clean$(RESET)            Remove Cargo build artifacts\n\n"
	@printf -- "$(GRAY)Rust $(RUST_VERSION) required. Browser tests require Playwright Chromium.$(RESET)\n"
