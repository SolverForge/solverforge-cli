const { test, expect } = require('@playwright/test');
const { readManifest } = require('./harness');

test.describe('Standard Solver Pipeline', () => {
  test('runs retained job pause/resume/cancel/delete through the generated UI', async ({ page }) => {
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
      expect(counts.resources).toBeGreaterThanOrEqual(8);
      expect(counts.tasks).toBeGreaterThanOrEqual(48);
    });

    await test.step('Solve, pause, inspect retained snapshot, cancel, and delete', async () => {
      const solveButton = page.getByRole('button', { name: 'Solve' });
      const pauseButton = page.getByRole('button', { name: 'Pause' });
      const resumeButton = page.getByRole('button', { name: 'Resume' });
      const cancelButton = page.getByRole('button', { name: 'Cancel' });

      await expect(solveButton).toBeVisible();
      await expect(pauseButton).toBeHidden();
      await expect(resumeButton).toBeHidden();
      await expect(cancelButton).toBeHidden();

      await solveButton.click();
      await expect(pauseButton).toBeVisible();
      await expect(cancelButton).toBeVisible();
      await expect(solveButton).toBeHidden();

      await page.waitForFunction(() => {
        const score = document.getElementById('sfScoreDisplay');
        const app = document.getElementById('sf-app');
        return !!score && score.textContent && score.textContent.trim() !== '—'
          && !!app && !!app.dataset.jobId;
      });

      const liveJobId = await page.locator('#sf-app').evaluate((el) => el.dataset.jobId);
      expect(liveJobId).toBeTruthy();

      await pauseButton.click();
      await page.waitForFunction(() => {
        const app = document.getElementById('sf-app');
        return !!app && app.dataset.lifecycleState === 'PAUSED' && !!app.dataset.snapshotRevision;
      });

      await expect(resumeButton).toBeVisible();
      await expect(pauseButton).toBeHidden();

      const paused = await page.evaluate(async () => {
        const app = document.getElementById('sf-app');
        const jobId = app.dataset.jobId;
        const snapshotRevision = app.dataset.snapshotRevision;
        const summary = await fetch(`/jobs/${jobId}`).then((response) => response.json());
        const snapshot = await fetch(`/jobs/${jobId}/snapshot?snapshot_revision=${snapshotRevision}`).then((response) => response.json());
        const analysis = await fetch(`/jobs/${jobId}/analysis?snapshot_revision=${snapshotRevision}`).then((response) => response.json());
        return { jobId, snapshotRevision: Number(snapshotRevision), summary, snapshot, analysis };
      });

      expect(paused.summary.lifecycleState).toBe('PAUSED');
      expect(paused.summary.checkpointAvailable).toBeTruthy();
      expect(paused.snapshot.snapshotRevision).toBe(paused.snapshotRevision);
      expect(Array.isArray(paused.snapshot.solution.resources)).toBeTruthy();
      expect(Array.isArray(paused.snapshot.solution.tasks)).toBeTruthy();
      expect(Array.isArray(paused.analysis.analysis.constraints)).toBeTruthy();

      await cancelButton.click();
      await page.waitForFunction(() => {
        const app = document.getElementById('sf-app');
        return !!app && app.dataset.lifecycleState === 'CANCELLED';
      });

      await expect(solveButton).toBeVisible({ timeout: 10000 });
      await expect(pauseButton).toBeHidden();
      await expect(resumeButton).toBeHidden();

      const cancelled = await page.evaluate(async (jobId) => {
        const summary = await fetch(`/jobs/${jobId}`).then((response) => response.json());
        const deleteResponse = await fetch(`/jobs/${jobId}`, { method: 'DELETE' });
        const afterDelete = await fetch(`/jobs/${jobId}`);
        return {
          summary,
          deleteStatus: deleteResponse.status,
          afterDeleteStatus: afterDelete.status,
        };
      }, liveJobId);

      expect(cancelled.summary.lifecycleState).toBe('CANCELLED');
      expect(cancelled.summary.terminalReason).toBe('cancelled');
      expect(cancelled.deleteStatus).toBe(204);
      expect(cancelled.afterDeleteStatus).toBe(404);
    });

    await test.step('Solve again and resume the same retained job', async () => {
      const solveButton = page.getByRole('button', { name: 'Solve' });
      const pauseButton = page.getByRole('button', { name: 'Pause' });
      const resumeButton = page.getByRole('button', { name: 'Resume' });

      await expect(solveButton).toBeVisible();
      await solveButton.click();
      await expect(pauseButton).toBeVisible();

      await page.waitForFunction(() => {
        const score = document.getElementById('sfScoreDisplay');
        const app = document.getElementById('sf-app');
        return !!score && score.textContent && score.textContent.trim() !== '—'
          && !!app && !!app.dataset.jobId;
      });

      const resumedJobId = await page.locator('#sf-app').evaluate((el) => el.dataset.jobId);
      expect(resumedJobId).toBeTruthy();

      await pauseButton.click();
      await page.waitForFunction((jobId) => {
        const app = document.getElementById('sf-app');
        return !!app
          && app.dataset.jobId === jobId
          && app.dataset.lifecycleState === 'PAUSED'
          && !!app.dataset.snapshotRevision;
      }, resumedJobId);

      await expect(resumeButton).toBeVisible();
      await resumeButton.click();

      await page.waitForFunction((jobId) => {
        const app = document.getElementById('sf-app');
        if (!app || app.dataset.jobId !== jobId) return false;
        return !!app.dataset.lifecycleState && app.dataset.lifecycleState !== 'PAUSED';
      }, resumedJobId);

      const resumed = await page.evaluate(async (jobId) => {
        const summary = await fetch(`/jobs/${jobId}`).then((response) => response.json());
        return {
          jobId,
          lifecycleState: summary.lifecycleState,
          summaryJobId: String(summary.id || summary.jobId || ''),
        };
      }, resumedJobId);

      expect(resumed.summaryJobId).toBe(resumed.jobId);
      expect(resumed.lifecycleState).not.toBe('PAUSED');
    });
  });
});
