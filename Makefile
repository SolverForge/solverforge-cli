# SolverForge CLI Makefile

GREEN := \033[92m
CYAN := \033[96m
YELLOW := \033[93m
RED := \033[91m
GRAY := \033[90m
BOLD := \033[1m
RESET := \033[0m

CHECK := OK
CROSS := XX
ARROW := >
PROGRESS := ->

VERSION := $(shell grep -m1 '^version' Cargo.toml | sed 's/version = "\(.*\)"/\1/')
BIN := solverforge

.PHONY: banner help build build-release run install test test-ignored test-unit test-one \
        lint fmt fmt-check clippy ci-local pre-release clean version

.DEFAULT_GOAL := help

banner:
	@printf -- "$(CYAN)$(BOLD)  SolverForge CLI$(RESET)\n"
	@printf -- "  $(GRAY)v$(VERSION)$(RESET) $(CYAN)Standalone project generator$(RESET)\n\n"

help: banner
	@printf -- "$(BOLD)Targets$(RESET)\n"
	@printf -- "  $(YELLOW)build$(RESET)          Build the CLI in debug mode\n"
	@printf -- "  $(YELLOW)build-release$(RESET)  Build the CLI in release mode\n"
	@printf -- "  $(YELLOW)run$(RESET)            Run the CLI locally; pass args with ARGS=\"...\"\n"
	@printf -- "  $(YELLOW)install$(RESET)        Install the CLI with cargo install --path .\n"
	@printf -- "  $(YELLOW)test$(RESET)           Run the default test suite\n"
	@printf -- "  $(YELLOW)test-ignored$(RESET)   Run ignored scaffold tests requiring network/toolchain access\n"
	@printf -- "  $(YELLOW)test-unit$(RESET)      Run unit tests in the $(BIN) binary target\n"
	@printf -- "  $(YELLOW)test-one$(RESET)       Run one test, e.g. make test-one TEST=scaffold_test\n"
	@printf -- "  $(YELLOW)lint$(RESET)           Run fmt-check and clippy\n"
	@printf -- "  $(YELLOW)ci-local$(RESET)       Simulate the main local validation flow\n"
	@printf -- "  $(YELLOW)pre-release$(RESET)    Validate formatting, linting, tests, and cargo publish --dry-run\n"
	@printf -- "  $(YELLOW)clean$(RESET)          Remove Cargo build artifacts\n\n"

build: banner
	@printf -- "$(ARROW) Building $(BIN)...\n"
	@cargo build && \
		printf -- "$(GREEN)$(CHECK) Build successful$(RESET)\n\n" || \
		(printf -- "$(RED)$(CROSS) Build failed$(RESET)\n\n" && exit 1)

build-release: banner
	@printf -- "$(ARROW) Building $(BIN) --release...\n"
	@cargo build --release && \
		printf -- "$(GREEN)$(CHECK) Release build successful$(RESET)\n\n" || \
		(printf -- "$(RED)$(CROSS) Release build failed$(RESET)\n\n" && exit 1)

run:
	@cargo run -- $(ARGS)

install:
	@printf -- "$(PROGRESS) Installing $(BIN) from current checkout...\n"
	@cargo install --path . && \
		printf -- "$(GREEN)$(CHECK) Installed$(RESET)\n" || \
		(printf -- "$(RED)$(CROSS) Install failed$(RESET)\n" && exit 1)

test: banner
	@printf -- "$(ARROW) Running cargo test...\n"
	@cargo test && \
		printf -- "\n$(GREEN)$(CHECK) Tests passed$(RESET)\n\n" || \
		(printf -- "\n$(RED)$(CROSS) Tests failed$(RESET)\n\n" && exit 1)

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

lint: banner fmt-check clippy
	@printf -- "\n$(GREEN)$(CHECK) Lint checks passed$(RESET)\n\n"

ci-local: banner
	@printf -- "$(PROGRESS) Step 1/4: format check...\n"
	@$(MAKE) fmt-check --no-print-directory
	@printf -- "$(PROGRESS) Step 2/4: build...\n"
	@cargo build --quiet && printf -- "$(GREEN)$(CHECK) Build passed$(RESET)\n"
	@printf -- "$(PROGRESS) Step 3/4: clippy...\n"
	@$(MAKE) clippy --no-print-directory
	@printf -- "$(PROGRESS) Step 4/4: tests...\n"
	@cargo test --quiet && printf -- "$(GREEN)$(CHECK) Tests passed$(RESET)\n"
	@printf -- "\n$(GREEN)$(CHECK) Local CI simulation passed$(RESET)\n\n"

pre-release: banner
	@$(MAKE) fmt-check --no-print-directory
	@$(MAKE) clippy --no-print-directory
	@printf -- "$(PROGRESS) Running cargo test...\n"
	@cargo test --quiet
	@printf -- "$(PROGRESS) Running cargo publish --dry-run...\n"
	@cargo publish --dry-run --allow-dirty
	@printf -- "\n$(GREEN)$(CHECK) Release checks passed for v$(VERSION)$(RESET)\n\n"

clean:
	@cargo clean
	@printf -- "$(GREEN)$(CHECK) Cleaned target artifacts$(RESET)\n"

version:
	@printf -- "$(CYAN)Current version:$(RESET) $(YELLOW)$(BOLD)$(VERSION)$(RESET)\n"
