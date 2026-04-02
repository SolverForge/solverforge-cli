const { test, expect } = require('@playwright/test');
const { readManifest } = require('./harness');

test.describe('Neutral Shell Pipeline', () => {
  test('loads the neutral shell from a fresh generated app', async ({ page }) => {
    const { scenarios } = readManifest();
    const scenario = scenarios.neutral;

    await test.step('Open generated app', async () => {
      await page.goto(scenario.baseUrl);
      await page.waitForSelector('#sf-app');
    });

    await test.step('Verify neutral overview state', async () => {
      await expect(page.getByText('No planning variables are declared yet')).toBeVisible();
      const uiModel = await page.evaluate(async () => {
        const response = await fetch('/generated/ui-model.json');
        return response.json();
      });
      expect(uiModel.views).toEqual([]);
      expect(uiModel.constraints).toEqual([]);
    });
  });
});
