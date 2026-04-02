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
      'standard',
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
  const standard = await scaffoldScenario('standard-solver', [
    ['generate', 'fact', 'resource'],
    ['generate', 'entity', 'task'],
    [
      'generate',
      'variable',
      'resource_idx',
      '--entity',
      'Task',
      '--kind',
      'standard',
      '--range',
      'resources',
      '--allows-unassigned',
    ],
  ]);

  writeManifest({
    scenarios: {
      neutral,
      mixed,
      standard,
    },
  });
  writeState({
    children: [neutral, mixed, standard].map((scenario) => ({
      name: scenario.name,
      pid: scenario.pid,
      tmpRoot: scenario.tmpRoot,
      artifactDir: scenario.artifactDir,
    })),
  });
};
