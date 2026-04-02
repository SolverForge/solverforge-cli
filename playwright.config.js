const { defineConfig } = require('@playwright/test');
const path = require('path');

module.exports = defineConfig({
  testDir: path.join(__dirname, 'tests', 'e2e'),
  outputDir: path.join(__dirname, 'target', 'test-artifacts', 'playwright', 'results'),
  fullyParallel: false,
  workers: 1,
  timeout: 60_000,
  reporter: [['list']],
  use: {
    browserName: 'chromium',
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
  globalSetup: path.join(__dirname, 'tests', 'e2e', 'global-setup.js'),
  globalTeardown: path.join(__dirname, 'tests', 'e2e', 'global-teardown.js'),
});
