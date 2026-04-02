const fs = require('fs');
const { readState, phase } = require('./harness');

module.exports = async function globalTeardown() {
  phase('playwright', 'Teardown generated browser scenarios');
  const keepArtifacts = process.env.KEEP_E2E_ARTIFACTS === '1';
  const state = readState();
  for (const child of state.children) {
    try {
      process.kill(child.pid, 'SIGTERM');
    } catch (_err) {}
    if (!keepArtifacts) {
      fs.rmSync(child.tmpRoot, { recursive: true, force: true });
    }
  }
};
