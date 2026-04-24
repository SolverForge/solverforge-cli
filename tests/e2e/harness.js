const fs = require('fs');
const os = require('os');
const path = require('path');
const net = require('net');
const { spawn, spawnSync } = require('child_process');

const repoRoot = path.resolve(__dirname, '..', '..');
const cliManifest = path.join(repoRoot, 'Cargo.toml');
const artifactRoot = process.env.SF_E2E_ARTIFACT_ROOT || path.join(repoRoot, 'target', 'test-artifacts', 'playwright');
const manifestPath = path.join(artifactRoot, 'manifest.json');
const statePath = path.join(artifactRoot, 'state.json');

const seededMixedDataModule = `/* Seeded demo data for Playwright end-to-end tests. */

use std::str::FromStr;

use crate::domain::{Container, Item, Plan, Resource, Task};

#[derive(Debug, Clone, Copy)]
pub enum DemoData {
    Small,
    Standard,
    Large,
}

const AVAILABLE_DEMO_DATA: &[DemoData] = &[DemoData::Small, DemoData::Standard, DemoData::Large];
const DEFAULT_DEMO_DATA: DemoData = DemoData::Standard;

pub fn default_demo_data() -> DemoData {
    DEFAULT_DEMO_DATA
}

pub fn available_demo_data() -> &'static [DemoData] {
    AVAILABLE_DEMO_DATA
}

impl DemoData {
    pub fn id(self) -> &'static str {
        match self {
            DemoData::Small => "SMALL",
            DemoData::Standard => "STANDARD",
            DemoData::Large => "LARGE",
        }
    }

    pub fn default_demo_data() -> Self {
        default_demo_data()
    }

    pub fn available_demo_data() -> &'static [Self] {
        available_demo_data()
    }
}

impl FromStr for DemoData {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "SMALL" => Ok(DemoData::Small),
            "STANDARD" => Ok(DemoData::Standard),
            "LARGE" => Ok(DemoData::Large),
            _ => Err(()),
        }
    }
}

pub fn generate(demo: DemoData) -> Plan {
    match demo {
        DemoData::Small => generate_plan(2, 3, 2, 4),
        DemoData::Standard => generate_plan(3, 6, 3, 8),
        DemoData::Large => generate_plan(6, 12, 6, 24),
    }
}

fn generate_plan(n_resources: usize, n_tasks: usize, n_containers: usize, n_items: usize) -> Plan {
    let resources = (0..n_resources)
        .map(|idx| Resource::new(format!("resource-{idx}"), format!("resource-{idx}")))
        .collect::<Vec<_>>();
    let tasks = (0..n_tasks)
        .map(|idx| Task::new(format!("task-{idx}")))
        .collect::<Vec<_>>();
    let items = (0..n_items)
        .map(|idx| Item::new(format!("item-{idx}"), format!("item-{idx}")))
        .collect::<Vec<_>>();
    let containers = (0..n_containers)
        .map(|idx| Container::new(format!("container-{idx}")))
        .collect::<Vec<_>>();

    Plan::new(resources, tasks, items, containers)
}
`;

const seededScalarDataModule = `/* Seeded scalar-variable demo data for Playwright end-to-end tests. */

use std::str::FromStr;

use crate::domain::{Plan, Resource, Task};

#[derive(Debug, Clone, Copy)]
pub enum DemoData {
    Small,
    Standard,
    Large,
}

const AVAILABLE_DEMO_DATA: &[DemoData] = &[DemoData::Small, DemoData::Standard, DemoData::Large];
const DEFAULT_DEMO_DATA: DemoData = DemoData::Standard;

pub fn default_demo_data() -> DemoData {
    DEFAULT_DEMO_DATA
}

pub fn available_demo_data() -> &'static [DemoData] {
    AVAILABLE_DEMO_DATA
}

impl DemoData {
    pub fn id(self) -> &'static str {
        match self {
            DemoData::Small => "SMALL",
            DemoData::Standard => "STANDARD",
            DemoData::Large => "LARGE",
        }
    }

    pub fn default_demo_data() -> Self {
        default_demo_data()
    }

    pub fn available_demo_data() -> &'static [Self] {
        available_demo_data()
    }
}

impl FromStr for DemoData {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "SMALL" => Ok(DemoData::Small),
            "STANDARD" => Ok(DemoData::Standard),
            "LARGE" => Ok(DemoData::Large),
            _ => Err(()),
        }
    }
}

pub fn generate(demo: DemoData) -> Plan {
    match demo {
        DemoData::Small => generate_plan(4, 16),
        DemoData::Standard => generate_plan(8, 48),
        DemoData::Large => generate_plan(16, 192),
    }
}

fn generate_plan(n_resources: usize, n_tasks: usize) -> Plan {
    let resources = (0..n_resources)
        .map(|idx| Resource::new(format!("resource-{idx}"), format!("resource-{idx}")))
        .collect::<Vec<_>>();
    let tasks = (0..n_tasks)
        .map(|idx| Task::new(format!("task-{idx}")))
        .collect::<Vec<_>>();

    Plan::new(resources, tasks)
}
`;

function phase(suite, title) {
  console.log(`=== PHASE: ${suite} :: ${title} ===`);
}

function ensureDir(dir) {
  fs.mkdirSync(dir, { recursive: true });
}

function workspaceRoot() {
  return path.dirname(repoRoot);
}

function tomlPath(value) {
  return value.replace(/\\/g, '/');
}

function usePublishedDeps() {
  const value = process.env.SF_USE_PUBLISHED_DEPS;
  return value && ['1', 'true', 'yes', 'on'].includes(value.toLowerCase());
}

function resolveBuiltExecutable(buildStdout) {
  let executable = null;
  for (const line of buildStdout.split(/\r?\n/)) {
    if (!line.trim()) {
      continue;
    }
    try {
      const message = JSON.parse(line);
      if (message.reason !== 'compiler-artifact') {
        continue;
      }
      if (!Array.isArray(message.target?.kind) || !message.target.kind.includes('bin')) {
        continue;
      }
      if (typeof message.executable === 'string' && message.executable.length > 0) {
        executable = message.executable;
      }
    } catch (_err) {}
  }
  if (!executable) {
    throw new Error('cargo build succeeded but did not report a binary executable');
  }
  return executable;
}

function resolveLocalSolverforgePaths() {
  if (usePublishedDeps()) {
    return null;
  }

  const runtimePath = path.join(workspaceRoot(), 'solverforge-rs', 'crates', 'solverforge');
  const uiPath = path.join(workspaceRoot(), 'solverforge-ui');
  const mapsPath = path.join(workspaceRoot(), 'solverforge-maps');
  if (!fs.existsSync(runtimePath) || !fs.existsSync(uiPath)) {
    return null;
  }

  return {
    runtimePath,
    uiPath,
    mapsPath: fs.existsSync(mapsPath) ? mapsPath : null,
  };
}

function pinGeneratedProjectToLocalSolverforge(projectDir) {
  const localPaths = resolveLocalSolverforgePaths();
  if (!localPaths) {
    return false;
  }

  const cargoTomlPath = path.join(projectDir, 'Cargo.toml');
  const cargoToml = fs.readFileSync(cargoTomlPath, 'utf8');
  const rewritten = cargoToml
    .split('\n')
    .map((line) => {
      const trimmed = line.trimStart();
      if (trimmed.startsWith('solverforge = ')) {
        return `solverforge = { path = "${tomlPath(localPaths.runtimePath)}", features = ["serde", "console", "verbose-logging"] }`;
      }
      if (trimmed.startsWith('solverforge-ui = ')) {
        return `solverforge-ui = { path = "${tomlPath(localPaths.uiPath)}" }`;
      }
      if (trimmed.startsWith('solverforge-maps = ') && localPaths.mapsPath) {
        return `solverforge-maps = { path = "${tomlPath(localPaths.mapsPath)}" }`;
      }
      return line;
    })
    .join('\n');
  fs.writeFileSync(cargoTomlPath, `${rewritten}\n`);
  return true;
}

function runCommand({ suite, title, cwd, args, logPath, env = {} }) {
  phase(suite, title);
  ensureDir(path.dirname(logPath));
  const result = spawnSync('cargo', ['run', '--quiet', '--manifest-path', cliManifest, '--bin', 'solverforge', '--', ...args], {
    cwd,
    env: { ...process.env, ...env },
    encoding: 'utf8',
  });
  fs.writeFileSync(
    logPath,
    `=== COMMAND ===\ncargo run --quiet --manifest-path ${cliManifest} --bin solverforge -- ${args.join(' ')}\n` +
      `=== STDOUT ===\n${result.stdout || ''}\n=== STDERR ===\n${result.stderr || ''}\n`
  );
  if (result.status !== 0) {
    throw new Error(`${title} failed.\nSee ${logPath}\n${result.stderr || result.stdout || ''}`);
  }
}

function allocatePort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.listen(0, '127.0.0.1', () => {
      const { port } = server.address();
      server.close(() => resolve(port));
    });
    server.on('error', reject);
  });
}

async function waitForHealth({ baseUrl, suite, child, stdoutPath, stderrPath }) {
  phase(suite, 'Wait for readiness');
  let spawnError = null;
  child.once('error', (error) => {
    spawnError = error;
  });
  const started = Date.now();
  while (Date.now() - started < 40000) {
    if (spawnError) {
      throw new Error(
        `Server failed to launch: ${spawnError.message}. stdout: ${stdoutPath} stderr: ${stderrPath}`
      );
    }
    if (child.exitCode !== null || child.signalCode !== null) {
      throw new Error(
        `Server exited before readiness (exitCode=${child.exitCode}, signal=${child.signalCode}). stdout: ${stdoutPath} stderr: ${stderrPath}`
      );
    }
    try {
      const response = await fetch(`${baseUrl}/health`);
      if (response.ok) {
        return;
      }
    } catch (_err) {}
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error(`Server at ${baseUrl} did not become healthy in time`);
}

async function scaffoldScenario(name, generatorCommands) {
  const suite = `playwright:${name}`;
  const tmpRoot = fs.mkdtempSync(path.join(os.tmpdir(), `solverforge-${name}-`));
  const projectName = `${name}-app`;
  const projectDir = path.join(tmpRoot, projectName);
  const scenarioArtifactDir = path.join(artifactRoot, name);
  ensureDir(scenarioArtifactDir);

  runCommand({
    suite,
    title: 'Scaffold neutral app',
    cwd: tmpRoot,
    args: ['new', projectName, '--skip-git', '--skip-readme', '--quiet'],
    logPath: path.join(scenarioArtifactDir, '01-scaffold.log'),
  });
  const pinnedToLocal = pinGeneratedProjectToLocalSolverforge(projectDir);
  if (!pinnedToLocal) {
    phase(suite, 'Using published SolverForge crate targets');
  }

  generatorCommands.forEach((args, index) => {
    runCommand({
      suite,
      title: `Generator command ${index + 1}`,
      cwd: projectDir,
      args,
      logPath: path.join(scenarioArtifactDir, `02-generate-${index + 1}.log`),
    });
  });

  if (name === 'mixed-pipeline') {
    phase(suite, 'Seed non-empty mixed demo data');
    fs.writeFileSync(path.join(projectDir, 'src', 'data', 'data_seed.rs'), seededMixedDataModule);
  }
  if (name === 'scalar-solver') {
    phase(suite, 'Seed non-empty scalar demo data');
    fs.writeFileSync(path.join(projectDir, 'src', 'data', 'data_seed.rs'), seededScalarDataModule);
  }

  phase(suite, 'Build generated app');
  const cargoBuild = spawnSync('cargo', ['build', '--message-format=json-render-diagnostics'], {
    cwd: projectDir,
    encoding: 'utf8',
  });
  let executablePath = null;
  let executableResolveError = null;
  if (cargoBuild.status === 0) {
    try {
      executablePath = resolveBuiltExecutable(cargoBuild.stdout || '');
    } catch (error) {
      executableResolveError = error;
    }
  }
  fs.writeFileSync(
    path.join(scenarioArtifactDir, '03-cargo-build.log'),
    `=== EXECUTABLE ===\n${executablePath || ''}\n=== STDOUT ===\n${cargoBuild.stdout || ''}\n=== STDERR ===\n${cargoBuild.stderr || ''}\n`
  );
  if (cargoBuild.status !== 0) {
    throw new Error(`cargo build failed for ${name}`);
  }
  if (executableResolveError) {
    throw new Error(
      `${executableResolveError.message}. See ${path.join(scenarioArtifactDir, '03-cargo-build.log')}`
    );
  }

  phase(suite, 'Boot generated server');
  const port = await allocatePort();
  const stdoutPath = path.join(scenarioArtifactDir, 'server.stdout.log');
  const stderrPath = path.join(scenarioArtifactDir, 'server.stderr.log');
  const stdout = fs.openSync(stdoutPath, 'w');
  const stderr = fs.openSync(stderrPath, 'w');
  const child = spawn(executablePath, [], {
    cwd: projectDir,
    env: { ...process.env, PORT: String(port) },
    stdio: ['ignore', stdout, stderr],
  });
  fs.closeSync(stdout);
  fs.closeSync(stderr);
  const baseUrl = `http://127.0.0.1:${port}`;
  await waitForHealth({ baseUrl, suite, child, stdoutPath, stderrPath });

  return {
    suite,
    name,
    tmpRoot,
    projectDir,
    artifactDir: scenarioArtifactDir,
    pid: child.pid,
    port,
    baseUrl,
  };
}

function writeManifest(manifest) {
  ensureDir(artifactRoot);
  fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2));
}

function writeState(state) {
  ensureDir(artifactRoot);
  fs.writeFileSync(statePath, JSON.stringify(state, null, 2));
}

function readManifest() {
  return JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
}

function readState() {
  return JSON.parse(fs.readFileSync(statePath, 'utf8'));
}

module.exports = {
  artifactRoot,
  manifestPath,
  phase,
  readManifest,
  readState,
  scaffoldScenario,
  writeManifest,
  writeState,
};
