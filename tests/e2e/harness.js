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
}

impl FromStr for DemoData {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "SMALL" => Ok(DemoData::Small),
            "STANDARD" => Ok(DemoData::Standard),
            _ => Err(()),
        }
    }
}

pub fn generate(demo: DemoData) -> Plan {
    match demo {
        DemoData::Small => generate_plan(2, 3, 2, 4),
        DemoData::Standard => generate_plan(3, 6, 3, 8),
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

const seededStandardDataModule = `/* Seeded standard-variable demo data for Playwright end-to-end tests. */

use std::str::FromStr;

use crate::domain::{Plan, Resource, Task};

#[derive(Debug, Clone, Copy)]
pub enum DemoData {
    Small,
    Standard,
}

impl FromStr for DemoData {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "SMALL" => Ok(DemoData::Small),
            "STANDARD" => Ok(DemoData::Standard),
            _ => Err(()),
        }
    }
}

pub fn generate(demo: DemoData) -> Plan {
    match demo {
        DemoData::Small => generate_plan(2, 4),
        DemoData::Standard => generate_plan(3, 6),
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

async function waitForHealth(baseUrl, suite) {
  phase(suite, 'Wait for readiness');
  const started = Date.now();
  while (Date.now() - started < 40000) {
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
    fs.writeFileSync(path.join(projectDir, 'src', 'data', 'mod.rs'), seededMixedDataModule);
  }
  if (name === 'standard-solver') {
    phase(suite, 'Seed non-empty standard demo data');
    fs.writeFileSync(path.join(projectDir, 'src', 'data', 'mod.rs'), seededStandardDataModule);
  }

  phase(suite, 'Compile generated app');
  const cargoCheck = spawnSync('cargo', ['check'], {
    cwd: projectDir,
    encoding: 'utf8',
  });
  fs.writeFileSync(
    path.join(scenarioArtifactDir, '03-cargo-check.log'),
    `=== STDOUT ===\n${cargoCheck.stdout || ''}\n=== STDERR ===\n${cargoCheck.stderr || ''}\n`
  );
  if (cargoCheck.status !== 0) {
    throw new Error(`cargo check failed for ${name}`);
  }

  phase(suite, 'Boot generated server');
  const port = await allocatePort();
  const stdoutPath = path.join(scenarioArtifactDir, 'server.stdout.log');
  const stderrPath = path.join(scenarioArtifactDir, 'server.stderr.log');
  const stdout = fs.openSync(stdoutPath, 'w');
  const stderr = fs.openSync(stderrPath, 'w');
  const child = spawn('cargo', ['run', '--quiet'], {
    cwd: projectDir,
    env: { ...process.env, PORT: String(port) },
    stdio: ['ignore', stdout, stderr],
  });
  const baseUrl = `http://127.0.0.1:${port}`;
  await waitForHealth(baseUrl, suite);

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
