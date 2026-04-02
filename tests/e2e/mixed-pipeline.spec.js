const { test, expect } = require('@playwright/test');
const { readManifest } = require('./harness');

test.describe('Mixed Pipeline', () => {
  test('drives the generated mixed app through browser-visible runtime phases', async ({ page }) => {
    const { scenarios } = readManifest();
    const scenario = scenarios.mixed;

    await test.step('Open generated mixed app', async () => {
      await page.goto(scenario.baseUrl);
      await page.waitForSelector('#sf-app');
    });

    await test.step('Verify generated view contract is mounted', async () => {
      const uiModel = await page.evaluate(async () => {
        const response = await fetch('/generated/ui-model.json');
        return response.json();
      });
      expect(uiModel.views.some((view) => view.kind === 'standard')).toBeTruthy();
      expect(uiModel.views.some((view) => view.kind === 'list')).toBeTruthy();
      for (const view of uiModel.views) {
        const exists = await page.$(`#view-${view.id}`);
        expect(exists).not.toBeNull();
      }
    });

    await test.step('Verify seeded mixed data is available to the generated app', async () => {
      const result = await page.evaluate(async () => {
        const demo = await fetch('/demo-data/STANDARD').then((response) => response.json());
        return {
          resources: (demo.resources || []).length,
          tasks: (demo.tasks || []).length,
          items: (demo.items || []).length,
          containers: (demo.containers || []).length,
        };
      });

      expect(result.resources).toBeGreaterThan(0);
      expect(result.tasks).toBeGreaterThan(0);
      expect(result.items).toBeGreaterThan(0);
      expect(result.containers).toBeGreaterThan(0);
    });

    await test.step('Reload page and verify generated views survive reconnect', async () => {
      await page.reload();
      await page.waitForSelector('#sf-app');
      await expect(page.getByRole('tab', { name: /REST API/ })).toBeVisible();
      await expect(page.getByRole('tab', { name: /Data/ })).toBeVisible();
    });
  });
});
