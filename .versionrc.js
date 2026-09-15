// Release configuration for commit-and-tag-version.
//
// The config declares every version surface the release tool owns. Besides the
// mechanical manifests, this includes the prose version lines in the docs and
// the bundled agent skill so no surface can drift from the package version.

// Builds an updater for a file whose version appears as the second capture
// group, wrapped by a stable prefix (first group) and suffix (third group).
const textVersion = (pattern) => ({
  readVersion(contents) {
    const match = contents.match(pattern);
    if (!match) {
      throw new Error(`version pattern not found: ${pattern}`);
    }
    return match[2];
  },
  writeVersion(contents, version) {
    return contents.replace(pattern, (_match, prefix, _current, suffix) => `${prefix}${version}${suffix}`);
  },
});

const cargoToml = textVersion(/(^version = ")(\d+\.\d+\.\d+)(")/m);
const cargoLock = textVersion(/(name = "solverforge-cli"\nversion = ")(\d+\.\d+\.\d+)(")/);
const readmeVersion = textVersion(/(Current CLI package version: `)(\d+\.\d+\.\d+)(`\.)/);
const agentsVersion = textVersion(/(- current CLI package version is `)(\d+\.\d+\.\d+)(`)/);
const wireframeVersion = readmeVersion;
const skillVersion = textVersion(/(This skill describes CLI `)(\d+\.\d+\.\d+)(`)/);

module.exports = {
  tagPrefix: 'v',
  releaseCommitMessageFormat: 'chore(release): {{currentTag}}',
  commitUrlFormat: 'https://github.com/SolverForge/solverforge-cli/commit/{{hash}}',
  compareUrlFormat: 'https://github.com/SolverForge/solverforge-cli/compare/{{previousTag}}...{{currentTag}}',
  packageFiles: [
    { filename: 'Cargo.toml', updater: cargoToml },
    { filename: 'package.json', type: 'json' },
  ],
  bumpFiles: [
    { filename: 'Cargo.toml', updater: cargoToml },
    { filename: 'Cargo.lock', updater: cargoLock },
    { filename: 'package.json', type: 'json' },
    { filename: 'package-lock.json', type: 'json' },
    { filename: 'README.md', updater: readmeVersion },
    { filename: 'AGENTS.md', updater: agentsVersion },
    { filename: 'WIREFRAME.md', updater: wireframeVersion },
    { filename: 'skills/solverforge-modeling/SKILL.md', updater: skillVersion },
  ],
};
