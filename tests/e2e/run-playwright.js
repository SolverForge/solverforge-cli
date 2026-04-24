const { spawnSync } = require('child_process');
const fs = require('fs');
const path = require('path');
const { artifactRoot } = require('./harness');

fs.mkdirSync(artifactRoot, { recursive: true });

const keepArtifacts =
  process.argv.includes('--keep-artifacts') || process.env.KEEP_E2E_ARTIFACTS === '1';

const childEnv = {
  ...process.env,
  SF_E2E_ARTIFACT_ROOT: artifactRoot,
};

if (keepArtifacts) {
  childEnv.KEEP_E2E_ARTIFACTS = '1';
}

if (childEnv.NO_COLOR) {
  delete childEnv.NO_COLOR;
}

const result = spawnSync(
  process.platform === 'win32' ? 'npx.cmd' : 'npx',
  ['playwright', 'test'],
  {
    stdio: 'inherit',
    env: childEnv,
  }
);

if (result.status === 0 && !keepArtifacts) {
  const manifest = path.join(artifactRoot, 'manifest.json');
  const state = path.join(artifactRoot, 'state.json');
  [manifest, state].forEach((file) => {
    if (fs.existsSync(file)) {
      fs.rmSync(file, { force: true });
    }
  });
}

process.exit(result.status ?? 1);
