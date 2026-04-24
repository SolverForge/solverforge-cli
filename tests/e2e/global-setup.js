const { scaffoldScenario, writeManifest, writeState, phase } = require('./harness');

module.exports = async function globalSetup() {
  phase('playwright', 'Scaffold browser scenarios');

  const neutral = await scaffoldScenario('neutral-shell', []);
  const mixed = await scaffoldScenario('mixed-pipeline', [
    ['generate', 'fact', 'resource'],
    ['generate', 'entity', 'task'],
    [
      'generate',
      'variable',
      'resource_idx',
      '--entity',
      'Task',
      '--kind',
      'scalar',
      '--range',
      'resources',
      '--allows-unassigned',
    ],
    ['generate', 'fact', 'item'],
    ['generate', 'entity', 'container'],
    [
      'generate',
      'variable',
      'item_order',
      '--entity',
      'Container',
      '--kind',
      'list',
      '--elements',
      'items',
    ],
  ]);
  const scalar = await scaffoldScenario('scalar-solver', [
    ['generate', 'fact', 'resource'],
    ['generate', 'entity', 'task'],
    [
      'generate',
      'variable',
      'resource_idx',
      '--entity',
      'Task',
      '--kind',
      'scalar',
      '--range',
      'resources',
      '--allows-unassigned',
    ],
  ]);

  writeManifest({
    scenarios: {
      neutral,
      mixed,
      scalar,
    },
  });
  writeState({
    children: [neutral, mixed, scalar].map((scenario) => ({
      name: scenario.name,
      pid: scenario.pid,
      tmpRoot: scenario.tmpRoot,
      artifactDir: scenario.artifactDir,
    })),
  });
};
