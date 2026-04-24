const { test, expect } = require('@playwright/test');
const { readManifest } = require('./harness');

test.describe('Scalar Solver Pipeline', () => {
  test('runs retained job pause/resume/stop/restart through the generated UI', async ({ page }) => {
    const { scenarios } = readManifest();
    const scenario = scenarios.scalar;
    let demoCatalog;
    let cancelledJobId;

    await test.step('Open generated scalar app', async () => {
      await page.goto(scenario.baseUrl);
      await page.waitForSelector('#sf-app');
      await expect(page.locator('#sf-app')).not.toHaveAttribute('data-bootstrap-error', 'true');
    });

    await test.step('Verify seeded scalar demo catalog and default data exist', async () => {
      demoCatalog = await page.evaluate(async () => {
        const response = await fetch('/demo-data');
        return response.json();
      });
      expect(demoCatalog.defaultId).toBe('STANDARD');
      expect(demoCatalog.availableIds).toEqual(['SMALL', 'STANDARD', 'LARGE']);

      const counts = await page.evaluate(async (defaultId) => {
        const demo = await fetch('/demo-data/' + defaultId).then((response) => response.json());
        return {
          resources: (demo.resources || []).length,
          tasks: (demo.tasks || []).length,
        };
      }, demoCatalog.defaultId);
      expect(counts.resources).toBeGreaterThanOrEqual(8);
      expect(counts.tasks).toBeGreaterThanOrEqual(48);
    });

    await test.step('Verify canonical timeline surface is rendered', async () => {
      await page.waitForSelector('.sf-rail-timeline');
      await expect(page.locator('.sf-rail-timeline')).toHaveCount(1);
    });

    await test.step('Verify REST API guide follows the live app origin', async () => {
      await page.getByRole('tab', { name: /REST API/ }).click();
      const commands = await page.locator('.sf-api-code-block code').allTextContents();
      expect(commands.length).toBeGreaterThanOrEqual(2);
      expect(commands[0]).toContain(`${scenario.baseUrl}/demo-data`);
      expect(commands[1]).toContain(`${scenario.baseUrl}/demo-data/${demoCatalog.defaultId}`);
      commands.forEach((command) => {
        expect(command).not.toContain('localhost:7860');
      });
    });

    await test.step('Solve, pause, inspect retained snapshot, and stop the retained job', async () => {
      const solveButton = page.getByRole('button', { name: 'Solve' });
      const pauseButton = page.getByRole('button', { name: 'Pause' });
      const resumeButton = page.getByRole('button', { name: 'Resume' });
      const stopButton = page.getByRole('button', { name: 'Stop' });

      await expect(solveButton).toBeVisible();
      await expect(solveButton).toBeEnabled();
      await expect(pauseButton).toBeHidden();
      await expect(resumeButton).toBeHidden();
      await expect(stopButton).toBeHidden();

      await solveButton.click();
      await expect(pauseButton).toBeVisible();
      await expect(stopButton).toBeVisible();
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

      await stopButton.click();
      await page.waitForFunction(() => {
        const app = document.getElementById('sf-app');
        return !!app && app.dataset.lifecycleState === 'CANCELLED';
      });

      await expect(solveButton).toBeVisible({ timeout: 10000 });
      await expect(pauseButton).toBeHidden();
      await expect(resumeButton).toBeHidden();

      const cancelled = await page.evaluate(async (jobId) => {
        const summary = await fetch(`/jobs/${jobId}`).then((response) => response.json());
        return { summary };
      }, liveJobId);

      expect(cancelled.summary.lifecycleState).toBe('CANCELLED');
      expect(cancelled.summary.terminalReason).toBe('cancelled');
      cancelledJobId = liveJobId;
    });

    await test.step('Solve again deletes terminal state before creating the next retained job', async () => {
      const solveButton = page.getByRole('button', { name: 'Solve' });
      const pauseButton = page.getByRole('button', { name: 'Pause' });
      const resumeButton = page.getByRole('button', { name: 'Resume' });
      const lifecycleRequests = [];
      page.on('request', (request) => {
        const url = new URL(request.url());
        if (url.pathname.startsWith('/jobs')) {
          lifecycleRequests.push({ method: request.method(), path: url.pathname });
        }
      });

      await expect(solveButton).toBeVisible();
      await solveButton.click();
      await expect(pauseButton).toBeVisible();

      await expect.poll(() => {
        const deleteIndex = lifecycleRequests.findIndex((entry) => (
          entry.method === 'DELETE' && entry.path === `/jobs/${cancelledJobId}`
        ));
        const postIndex = lifecycleRequests.findIndex((entry, index) => (
          index > deleteIndex && entry.method === 'POST' && entry.path === '/jobs'
        ));
        return deleteIndex !== -1 && postIndex !== -1;
      }).toBe(true);

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
