const { test, expect } = require('@playwright/test');
const { readManifest } = require('./harness');

test.describe('Mixed Pipeline', () => {
  test('drives the generated mixed app through browser-visible runtime phases', async ({ page }) => {
    const { scenarios } = readManifest();
    const scenario = scenarios.mixed;
    let generatedViewCount = 0;
    let demoCatalog;

    await test.step('Open generated mixed app', async () => {
      await page.goto(scenario.baseUrl);
      await page.waitForSelector('#sf-app');
      await expect(page.locator('#sf-app')).not.toHaveAttribute('data-bootstrap-error', 'true');
    });

    await test.step('Verify generated view contract is mounted', async () => {
      const uiModel = await page.evaluate(async () => {
        const response = await fetch('/generated/ui-model.json');
        return response.json();
      });
      expect(uiModel.views.some((view) => view.kind === 'scalar')).toBeTruthy();
      expect(uiModel.views.some((view) => view.kind === 'list')).toBeTruthy();
      generatedViewCount = uiModel.views.length;
      for (const view of uiModel.views) {
        const exists = await page.$(`#view-${view.id}`);
        expect(exists).not.toBeNull();
      }
    });

    await test.step('Verify seeded mixed data discovery is available to the generated app', async () => {
      demoCatalog = await page.evaluate(async () => {
        const response = await fetch('/demo-data');
        return response.json();
      });
      expect(demoCatalog.defaultId).toBe('STANDARD');
      expect(demoCatalog.availableIds).toEqual(['SMALL', 'STANDARD', 'LARGE']);

      const result = await page.evaluate(async (defaultId) => {
        const demo = await fetch('/demo-data/' + defaultId).then((response) => response.json());
        return {
          resources: (demo.resources || []).length,
          tasks: (demo.tasks || []).length,
          items: (demo.items || []).length,
          containers: (demo.containers || []).length,
        };
      }, demoCatalog.defaultId);

      expect(result.resources).toBeGreaterThan(0);
      expect(result.tasks).toBeGreaterThan(0);
      expect(result.items).toBeGreaterThan(0);
      expect(result.containers).toBeGreaterThan(0);
    });

    await test.step('Verify canonical timeline surface mounts for generated views', async () => {
      await page.waitForSelector('.sf-rail-timeline');
      await expect(page.locator('.sf-rail-timeline')).toHaveCount(generatedViewCount);
    });

    await test.step('Verify lifecycle controls stay stock and expose Stop for runtime cancel', async () => {
      await expect(page.getByRole('button', { name: 'Solve' })).toBeVisible();
      await expect(page.getByRole('button', { name: 'Pause' })).toBeHidden();
      await expect(page.getByRole('button', { name: 'Resume' })).toBeHidden();
      await expect(page.getByRole('button', { name: 'Stop' })).toBeHidden();

      const appSource = await page.evaluate(async () => {
        return fetch('/app.js').then((response) => response.text());
      });
      expect(appSource).toContain('SF.createSolver');
      expect(appSource).toContain('cleanupTerminalJob()');
      expect(appSource).toContain('solver.delete()');
      expect(appSource).not.toContain('last_event');
      expect(appSource).not.toContain('lastEvent');
      expect(appSource).not.toContain('sse_snapshot');
      expect(appSource).not.toContain('sseSnapshot');
    });

    await test.step('Reload page and verify generated views survive reconnect', async () => {
      await page.reload();
      await page.waitForSelector('#sf-app');
      await expect(page.getByRole('tab', { name: /REST API/ })).toBeVisible();
      await expect(page.getByRole('tab', { name: /Data/ })).toBeVisible();
    });
  });
});
