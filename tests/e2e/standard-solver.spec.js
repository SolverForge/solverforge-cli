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

    await test.step('Create a solve and observe typed SSE', async () => {
      const result = await page.evaluate(async () => {
        const demo = await fetch('/demo-data/STANDARD').then((response) => response.json());
        const create = await fetch('/schedules', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(demo),
        }).then((response) => response.json());

        const eventTypes = [];
        await new Promise((resolve, reject) => {
          const events = new EventSource(`/schedules/${create.id}/events`);
          const timeout = setTimeout(() => {
            events.close();
            resolve();
          }, 5000);
          events.onmessage = (event) => {
            const payload = JSON.parse(event.data);
            eventTypes.push(payload.eventType);
            if (payload.eventType === 'finished') {
              clearTimeout(timeout);
              events.close();
              resolve();
            }
          };
          events.onerror = () => {
            clearTimeout(timeout);
            events.close();
            reject(new Error('SSE stream failed'));
          };
        });

        const schedule = await fetch(`/schedules/${create.id}`).then((response) => response.json());
        const status = await fetch(`/schedules/${create.id}/status`).then((response) => response.json());
        const analysis = await fetch(`/schedules/${create.id}/analyze`).then((response) => response.json());
        return { id: create.id, eventTypes, schedule, status, analysis };
      });

      expect(result.id).toBeTruthy();
      expect(result.eventTypes.length).toBeGreaterThan(0);
      expect(result.eventTypes.every((eventType) => ['progress', 'best_solution', 'finished'].includes(eventType))).toBeTruthy();
      expect(Array.isArray(result.schedule.resources)).toBeTruthy();
      expect(result.schedule.resources.length).toBeGreaterThan(0);
      expect(Array.isArray(result.schedule.tasks)).toBeTruthy();
      expect(result.schedule.tasks.length).toBeGreaterThan(0);
      expect(result.status.solverStatus).toBeTruthy();
      expect(Array.isArray(result.analysis.constraints)).toBeTruthy();
    });
  });
});
