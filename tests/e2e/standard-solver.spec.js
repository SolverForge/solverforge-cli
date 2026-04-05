const { test, expect } = require('@playwright/test');
const { readManifest } = require('./harness');

test.describe('Standard Solver Pipeline', () => {
  test('runs seeded data through the real generated solver from the browser side', async ({ page }) => {
    const { scenarios } = readManifest();
    const scenario = scenarios.standard;

    await test.step('Open generated standard app', async () => {
      await page.goto(scenario.baseUrl);
      await page.waitForSelector('#sf-app');
    });

    await test.step('Verify seeded standard demo data exists', async () => {
      const counts = await page.evaluate(async () => {
        const demo = await fetch('/demo-data/STANDARD').then((response) => response.json());
        return {
          resources: (demo.resources || []).length,
          tasks: (demo.tasks || []).length,
        };
      });
      expect(counts.resources).toBeGreaterThan(0);
      expect(counts.tasks).toBeGreaterThan(0);
    });

    await test.step('Solve, stop, and resume through the generated UI', async () => {
      const solveButton = page.getByRole('button', { name: 'Solve' });
      const stopButton = page.getByRole('button', { name: 'Stop' });

      await expect(solveButton).toBeVisible();
      await expect(stopButton).toBeHidden();

      await solveButton.click();
      await expect(stopButton).toBeVisible();
      await expect(solveButton).toBeHidden();

      await page.waitForFunction(() => {
        const score = document.getElementById('sfScoreDisplay');
        return !!score && score.textContent && score.textContent.trim() !== '—';
      });

      await stopButton.click();
      await expect(solveButton).toBeVisible({ timeout: 10000 });
      await expect(stopButton).toBeHidden();

      const afterStop = await page.evaluate(async () => {
        const ids = await fetch('/schedules').then((response) => response.json());
        const latestId = ids[ids.length - 1];
        const schedule = await fetch(`/schedules/${latestId}`).then((response) => response.json());
        const status = await fetch(`/schedules/${latestId}/status`).then((response) => response.json());
        return { latestId, schedule, status };
      });

      expect(afterStop.latestId).toBeTruthy();
      expect(afterStop.status.solverStatus).toBe('NOT_SOLVING');
      expect(Array.isArray(afterStop.schedule.resources)).toBeTruthy();
      expect(Array.isArray(afterStop.schedule.tasks)).toBeTruthy();

      await solveButton.click();
      await expect(stopButton).toBeVisible();
      await stopButton.click();
      await expect(solveButton).toBeVisible({ timeout: 10000 });

      const result = await page.evaluate(async () => {
        const ids = await fetch('/schedules').then((response) => response.json());
        const latestId = ids[ids.length - 1];
        const schedule = await fetch(`/schedules/${latestId}`).then((response) => response.json());
        const status = await fetch(`/schedules/${latestId}/status`).then((response) => response.json());
        const analysis = await fetch(`/schedules/${latestId}/analyze`).then((response) => response.json());
        return { latestId, schedule, status, analysis, jobCount: ids.length };
      });

      expect(result.jobCount).toBeGreaterThan(1);
      expect(result.latestId).toBeTruthy();
      expect(result.status.solverStatus).toBe('NOT_SOLVING');
      expect(Array.isArray(result.schedule.resources)).toBeTruthy();
      expect(result.schedule.resources.length).toBeGreaterThan(0);
      expect(Array.isArray(result.schedule.tasks)).toBeTruthy();
      expect(result.schedule.tasks.length).toBeGreaterThan(0);
      expect(Array.isArray(result.analysis.constraints)).toBeTruthy();
    });
  });
});
