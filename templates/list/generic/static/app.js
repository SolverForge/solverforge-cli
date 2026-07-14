/* app.js — {{project_name}} SolverForge UI */

(async function () {
  'use strict';

  var SLOT_MINUTES = 60;
  var DEFAULT_VIEWPORT_SLOTS = 12;
  var TIMELINE_TONES = ['emerald', 'blue', 'amber', 'rose', 'violet', 'slate'];

  var config = await fetch('/sf-config.json').then(function (response) { return response.json(); });

  var app = document.getElementById('sf-app');
  var currentPlan = null;
  var lastAnalysis = null;
  var bootstrapError = null;
  var demoCatalog = { defaultId: null, availableIds: [] };
  var sequenceTimeline = null;

  var backend = SF.createBackend({ baseUrl: '' });
  var statusBar = SF.createStatusBar({ constraints: config.constraints || [] });
  var solver = SF.createSolver({
    backend: backend,
    statusBar: statusBar,
    onProgress: function (meta) {
      syncLifecycleMarkers(meta);
    },
    onPauseRequested: function (meta) {
      syncLifecycleMarkers(meta);
    },
    onSolution: function (snapshot, meta) {
      renderAll(snapshot && snapshot.solution ? snapshot.solution : null);
      syncLifecycleMarkers(meta);
    },
    onPaused: function (snapshot, meta) {
      renderAll(snapshot && snapshot.solution ? snapshot.solution : null);
      syncLifecycleMarkers(meta);
    },
    onResumed: function (meta) {
      syncLifecycleMarkers(meta);
    },
    onCancelled: function (snapshot, meta) {
      renderAll(snapshot && snapshot.solution ? snapshot.solution : null);
      syncLifecycleMarkers(meta);
    },
    onComplete: function (snapshot, meta) {
      renderAll(snapshot && snapshot.solution ? snapshot.solution : null);
      syncLifecycleMarkers(meta);
    },
    onFailure: function (message, meta, snapshot, analysis) {
      renderAll(snapshot && snapshot.solution ? snapshot.solution : null);
      if (analysis) {
        lastAnalysis = analysis;
      }
      console.error('Solver job failed:', message);
      syncLifecycleMarkers(meta);
    },
    onAnalysis: function (analysis) {
      lastAnalysis = analysis;
      syncLifecycleMarkers();
    },
    onError: function (message) {
      console.error('Solver lifecycle failed:', message);
      syncLifecycleMarkers();
    },
  });

  var header = SF.createHeader({
    logo: '/sf/img/ouroboros.svg',
    title: config.title,
    subtitle: config.subtitle,
    tabs: [
      { id: 'timeline', label: 'Timeline', icon: 'fa-list-ol', active: true },
      { id: 'data', label: 'Data', icon: 'fa-table' },
      { id: 'api', label: 'REST API', icon: 'fa-book' },
    ],
    actions: {
      onSolve: function () { loadAndSolve(); },
      onPause: function () { pauseSolve(); },
      onResume: function () { resumeSolve(); },
      onCancel: function () { cancelSolve(); },
      onAnalyze: function () { openAnalysis(); },
    },
    onTabChange: function (tab) {
      sequencesPanel.style.display = tab === 'timeline' ? '' : 'none';
      dataPanel.style.display = tab === 'data' ? '' : 'none';
      apiPanel.style.display = tab === 'api' ? '' : 'none';
    },
  });
  app.appendChild(header);
  statusBar.bindHeader(header);
  app.appendChild(statusBar.el);

  var bootstrapNotice = SF.el('div', {
    className: 'sf-content',
    style: {
      display: 'none',
      padding: '16px',
      marginBottom: '16px',
      borderRadius: '12px',
      border: '1px solid #dc2626',
      background: '#fef2f2',
      color: '#991b1b',
    },
  });
  app.appendChild(bootstrapNotice);

  var sequencesPanel = SF.el('div', { className: 'sf-content' });
  var sequencesContainer = SF.el('div', { id: 'sf-sequences' });
  sequencesPanel.appendChild(sequencesContainer);
  app.appendChild(sequencesPanel);

  var dataPanel = SF.el('div', { className: 'sf-content', style: { display: 'none' } });
  var tablesContainer = SF.el('div', { id: 'sf-tables' });
  dataPanel.appendChild(tablesContainer);
  app.appendChild(dataPanel);

  var apiPanel = SF.el('div', { className: 'sf-content', style: { display: 'none' } });
  var apiGuideContainer = SF.el('div');
  apiPanel.appendChild(apiGuideContainer);
  app.appendChild(apiPanel);

  app.appendChild(SF.createFooter({
    links: [
      { label: 'SolverForge', url: 'https://www.solverforge.org' },
      { label: 'Docs', url: 'https://www.solverforge.org/docs' },
    ],
  }));

  var analysisModal = SF.createModal({ title: 'Score Analysis', width: '700px' });
  renderApiGuide();
  updateSolveActionAvailability();
  bootstrapDemoData();

  window.addEventListener('beforeunload', destroySequenceTimeline);

  function loadAndSolve() {
    if (solver.isRunning() || solver.getLifecycleState() === 'PAUSED' || !canSolve()) return;
    cleanupTerminalJob()
      .then(function (data) {
        if (data) return data;
        if (currentPlan) return clonePlan(currentPlan);
        if (!demoCatalog.defaultId) {
          return Promise.reject(new Error('demo data catalog is unavailable'));
        }
        return fetchDemoPlan(demoCatalog.defaultId);
      })
      .then(function (data) {
        return solver.start(clonePlan(data));
      })
      .then(function () {
        syncLifecycleMarkers();
      })
      .catch(function (err) { console.error('Demo load failed:', err); });
  }

  function pauseSolve() {
    solver.pause()
      .then(function () { syncLifecycleMarkers(); })
      .catch(function (err) { console.error('Pause failed:', err); });
  }

  function resumeSolve() {
    solver.resume()
      .then(function () { syncLifecycleMarkers(); })
      .catch(function (err) { console.error('Resume failed:', err); });
  }

  function cancelSolve() {
    solver.cancel()
      .then(function () { syncLifecycleMarkers(); })
      .catch(function (err) { console.error('Cancel failed:', err); });
  }

  function cleanupTerminalJob() {
    var state = solver.getLifecycleState();
    var jobId = solver.getJobId();
    if (jobId == null || jobId === '' || state === 'IDLE' || state === 'PAUSED' || solver.isRunning()) {
      return Promise.resolve(null);
    }
    return solver.delete()
      .then(function () {
        lastAnalysis = null;
        syncLifecycleMarkers();
        return null;
      })
      .catch(function (err) {
        console.error('Delete failed:', err);
        throw err;
      });
  }

  function openAnalysis() {
    var id = solver.getJobId();
    if (id == null || id === '') return;
    solver.analyzeSnapshot()
      .then(function (analysis) {
        lastAnalysis = analysis;
        analysisModal.setBody(buildAnalysisBody(analysis));
        analysisModal.open();
      })
      .catch(function () {});
  }

  function renderAll(data) {
    if (!data) return;
    currentPlan = clonePlan(data);
    renderSequences(data);
    renderTables(data);
  }

  function bootstrapDemoData() {
    fetchDemoCatalog()
      .then(function (catalog) {
        demoCatalog = catalog;
        clearBootstrapError();
        renderApiGuide();
        return fetchDemoPlan(catalog.defaultId);
      })
      .then(function (data) {
        renderAll(data);
        updateSolveActionAvailability();
      })
      .catch(function (err) {
        reportBootstrapError(err);
      });
  }

  function fetchDemoCatalog() {
    return requestJson('/demo-data', 'demo data catalog')
      .then(function (catalog) {
        if (!catalog || typeof catalog.defaultId !== 'string' || !Array.isArray(catalog.availableIds)) {
          throw new Error('demo data catalog is missing defaultId or availableIds');
        }
        if (catalog.availableIds.indexOf(catalog.defaultId) === -1) {
          throw new Error('demo data catalog defaultId is not present in availableIds');
        }
        return {
          defaultId: catalog.defaultId,
          availableIds: catalog.availableIds.slice(),
        };
      });
  }

  function fetchDemoPlan(demoId) {
    return requestJson('/demo-data/' + encodeURIComponent(demoId), 'demo data "' + demoId + '"');
  }

  function requestJson(path, label) {
    return fetch(path)
      .then(function (response) {
        if (!response.ok) {
          throw new Error(label + ' returned HTTP ' + response.status);
        }
        return response.json();
      });
  }

  function canSolve() {
    return !bootstrapError && !!demoCatalog.defaultId;
  }

  function reportBootstrapError(err) {
    bootstrapError = describeError(err);
    bootstrapNotice.textContent = 'Demo data bootstrap failed: ' + bootstrapError;
    bootstrapNotice.style.display = '';
    app.dataset.bootstrapError = 'true';
    renderApiGuide();
    updateSolveActionAvailability();
    console.error('Demo data bootstrap failed:', err);
  }

  function clearBootstrapError() {
    bootstrapError = null;
    bootstrapNotice.textContent = '';
    bootstrapNotice.style.display = 'none';
    delete app.dataset.bootstrapError;
  }

  function describeError(err) {
    if (err && err.message) {
      return err.message;
    }
    return String(err || 'unknown error');
  }

  function updateSolveActionAvailability() {
    var solveButton = findHeaderButton('Solve');
    var disabled = !canSolve();
    if (!solveButton) return;
    solveButton.disabled = disabled;
    solveButton.setAttribute('aria-disabled', disabled ? 'true' : 'false');
    solveButton.title = disabled
      ? (bootstrapError ? 'Demo data bootstrap failed.' : 'Loading demo data catalog...')
      : '';
  }

  function findHeaderButton(label) {
    var buttons = header.querySelectorAll('button');
    for (var i = 0; i < buttons.length; i += 1) {
      var text = (buttons[i].textContent || '').trim();
      if (text === label) {
        return buttons[i];
      }
    }
    return null;
  }

  function renderApiGuide() {
    apiGuideContainer.innerHTML = '';
    apiGuideContainer.appendChild(SF.createApiGuide({
      endpoints: buildApiGuideEndpoints(),
    }));
  }

  function buildApiGuideEndpoints() {
    var defaultDemoPath = demoCatalog.defaultId
      ? '/demo-data/' + demoCatalog.defaultId
      : '/demo-data/{defaultId}';
    return [
      { method: 'GET', path: '/demo-data', description: 'Discover the default and available demo data IDs', curl: buildCurlCommand('GET', '/demo-data') },
      { method: 'GET', path: defaultDemoPath, description: 'Fetch the discovered default demo data', curl: buildCurlCommand('GET', defaultDemoPath) },
      { method: 'POST', path: '/jobs', description: 'Create a retained solving job', curl: buildCurlCommand('POST', '/jobs', { json: true, data: '@plan.json' }) },
      { method: 'POST', path: '/jobs/qualified', description: 'Create a retained job with qualified candidate-trace provenance', curl: buildCurlCommand('POST', '/jobs/qualified', { json: true, data: '@qualified-plan.json' }) },
      { method: 'GET', path: '/jobs/{id}', description: 'Get current job summary', curl: buildCurlCommand('GET', '/jobs/{id}') },
      { method: 'GET', path: '/jobs/{id}/snapshot', description: 'Fetch the latest retained snapshot', curl: buildCurlCommand('GET', '/jobs/{id}/snapshot') },
      { method: 'GET', path: '/jobs/{id}/analysis?snapshot_revision={n}', description: 'Analyze an exact snapshot revision', curl: buildCurlCommand('GET', '/jobs/{id}/analysis?snapshot_revision=3', { quoteUrl: true }) },
      { method: 'GET', path: '/jobs/{id}/telemetry', description: 'Fetch atomically retained aggregate and candidate-trace diagnostics', curl: buildCurlCommand('GET', '/jobs/{id}/telemetry') },
      { method: 'POST', path: '/jobs/{id}/pause', description: 'Request an exact runtime pause', curl: buildCurlCommand('POST', '/jobs/{id}/pause') },
      { method: 'POST', path: '/jobs/{id}/resume', description: 'Resume a paused retained job', curl: buildCurlCommand('POST', '/jobs/{id}/resume') },
      { method: 'POST', path: '/jobs/{id}/cancel', description: 'Cancel a live or paused job', curl: buildCurlCommand('POST', '/jobs/{id}/cancel') },
      { method: 'DELETE', path: '/jobs/{id}', description: 'Delete a terminal retained job', curl: buildCurlCommand('DELETE', '/jobs/{id}') },
      { method: 'GET', path: '/jobs/{id}/events', description: 'Stream job lifecycle updates (SSE)', curl: buildCurlCommand('GET', '/jobs/{id}/events', { stream: true }) },
    ];
  }

  function buildCurlCommand(method, path, options) {
    var parts = ['curl'];
    if (options && options.stream) {
      parts.push('-N');
    }
    if (method && method !== 'GET') {
      parts.push('-X', method);
    }
    if (options && options.json) {
      parts.push('-H', '"Content-Type: application/json"');
    }

    var url = buildApiUrl(path);
    parts.push(options && options.quoteUrl ? '"' + url + '"' : url);

    if (options && options.data) {
      parts.push('-d', options.data);
    }

    return parts.join(' ');
  }

  function buildApiUrl(path) {
    return currentOrigin() + path;
  }

  function currentOrigin() {
    return window.location.origin || (window.location.protocol + '//' + window.location.host);
  }

  function syncLifecycleMarkers(meta) {
    var jobId = solver.getJobId();
    var snapshotRevision = solver.getSnapshotRevision();
    var lifecycleState = meta && meta.lifecycleState ? meta.lifecycleState : solver.getLifecycleState();

    if (jobId) {
      app.dataset.jobId = String(jobId);
    } else {
      delete app.dataset.jobId;
    }
    if (snapshotRevision != null) {
      app.dataset.snapshotRevision = String(snapshotRevision);
    } else {
      delete app.dataset.snapshotRevision;
    }
    if (lifecycleState && lifecycleState !== 'IDLE') {
      app.dataset.lifecycleState = lifecycleState;
    } else {
      delete app.dataset.lifecycleState;
    }
    updateSolveActionAvailability();
  }

  function buildAnalysisBody(analysis) {
    var container = SF.el('div');
    if (!analysis || !analysis.constraints) {
      container.appendChild(SF.el('p', null, 'No analysis available.'));
      return container;
    }

    container.appendChild(SF.el('p', null, SF.el('strong', null, 'Score: '), String(analysis.score)));
    container.appendChild(SF.createTable({
      columns: ['Constraint', 'Type', 'Score', 'Matches'],
      rows: analysis.constraints.map(function (constraint) {
        var matchCount = constraint.matchCount != null ? constraint.matchCount : (constraint.matches ? constraint.matches.length : 0);
        return [
          constraint.name,
          constraint.constraintType || constraint.type || '',
          constraint.score,
          String(matchCount),
        ];
      }),
    }));
    return container;
  }

  function clonePlan(data) {
    return JSON.parse(JSON.stringify(data));
  }

  function renderSequences(data) {
    sequencesContainer.innerHTML = '';
    var payload = buildSequencePayload(data);
    if (!payload) return;

    sequencesContainer.appendChild(payload.summary);
    sequencesContainer.appendChild(ensureSequenceTimeline(payload.timeline).el);
  }

  function ensureSequenceTimeline(timelineConfig) {
    if (!sequenceTimeline) {
      sequenceTimeline = SF.rail.createTimeline(timelineConfig);
      return sequenceTimeline;
    }

    sequenceTimeline.setModel(timelineConfig.model);
    return sequenceTimeline;
  }

  function destroySequenceTimeline() {
    if (!sequenceTimeline) return;
    sequenceTimeline.destroy();
    sequenceTimeline = null;
  }

  function buildSequencePayload(data) {
    var containers = data.containers || [];
    if (!containers.length) return null;

    var itemsById = buildItemsById(data);
    var metrics = deriveSequenceMetrics(containers);
    var sortedContainers = containers.slice().sort(compareContainers);
    var horizon = Math.max(metrics.longestSequence, 1);
    var axis = buildSlotAxis(horizon);

    var lanes = sortedContainers.map(function (container) {
      var sequence = container.items || [];
      var firstItem = sequence.length ? describeItem(sequence[0], itemsById).name : '—';
      var lastItem = sequence.length ? describeItem(sequence[sequence.length - 1], itemsById).name : '—';
      return {
        id: 'container-' + String(container.id != null ? container.id : container.name),
        label: container.name || 'Unnamed container',
        mode: 'detailed',
        badges: containerBadges(sequence.length, metrics.longestSequence),
        stats: [
          { label: 'Items', value: sequence.length },
          { label: 'First', value: firstItem },
          { label: 'Last', value: lastItem },
        ],
        items: sequence.map(function (itemId, index) {
          var item = describeItem(itemId, itemsById);
          return buildTimelineItem(
            'container-' + String(container.id || container.name) + '-item-' + String(index),
            index,
            item.name,
            'Position ' + String(index + 1),
            item.key
          );
        }),
      };
    });

    return {
      summary: buildSummarySection(
        ['Containers', 'Items', 'Longest sequence', 'Empty containers', 'Average items / container'],
        [
          String(metrics.totalContainers),
          String(metrics.totalItems),
          String(metrics.longestSequence),
          String(metrics.emptyContainers),
          String(metrics.averageItems),
        ]
      ),
      timeline: {
        label: config.entities[0] ? config.entities[0].label : 'Container',
        labelWidth: 280,
        title: config.title,
        subtitle: 'Canonical retained timeline for list-variable ownership sequences',
        model: {
          axis: axis,
          lanes: lanes,
        },
      },
    };
  }

  function buildItemsById(data) {
    var items = data.items || data.itemFacts || data.item_facts || [];
    return items.reduce(function (map, item) {
      if (item && item.id) map[item.id] = item;
      return map;
    }, {});
  }

  function deriveSequenceMetrics(containers) {
    var lengths = containers.map(function (container) {
      return (container.items || []).length;
    });
    var totalItems = lengths.reduce(function (sum, count) { return sum + count; }, 0);
    var longestSequence = lengths.reduce(function (maxCount, count) {
      return Math.max(maxCount, count);
    }, 0);
    var emptyContainers = lengths.filter(function (count) { return count === 0; }).length;
    return {
      totalContainers: containers.length,
      totalItems: totalItems,
      longestSequence: longestSequence,
      emptyContainers: emptyContainers,
      averageItems: containers.length ? (totalItems / containers.length).toFixed(1) : '0.0',
    };
  }

  function compareContainers(left, right) {
    var leftCount = (left.items || []).length;
    var rightCount = (right.items || []).length;
    if (rightCount !== leftCount) return rightCount - leftCount;
    return String(left.name || '').localeCompare(String(right.name || ''));
  }

  function buildSummarySection(columns, row) {
    var section = SF.el('div', { className: 'sf-section' });
    section.appendChild(SF.createTable({
      columns: columns,
      rows: [row],
    }));
    return section;
  }

  function buildSlotAxis(slotCount) {
    var normalizedSlots = Math.max(slotCount, 1);
    var groupSize = normalizedSlots > 24 ? 8 : (normalizedSlots > 12 ? 6 : 4);
    var days = [];
    var ticks = [];

    for (var startSlot = 0; startSlot < normalizedSlots; startSlot += groupSize) {
      var endSlot = Math.min(normalizedSlots, startSlot + groupSize);
      days.push({
        id: 'window-' + startSlot,
        label: 'Window ' + String(days.length + 1),
        subLabel: slotRangeLabel(startSlot, endSlot),
        startMinute: startSlot * SLOT_MINUTES,
        endMinute: endSlot * SLOT_MINUTES,
      });
    }

    for (var slotIndex = 0; slotIndex < normalizedSlots; slotIndex += 1) {
      ticks.push({
        id: 'tick-' + slotIndex,
        minute: slotIndex * SLOT_MINUTES,
        label: 'Slot ' + String(slotIndex + 1),
      });
    }

    return {
      startMinute: 0,
      endMinute: normalizedSlots * SLOT_MINUTES,
      days: days,
      ticks: ticks,
      initialViewport: {
        startMinute: 0,
        endMinute: Math.min(normalizedSlots, DEFAULT_VIEWPORT_SLOTS) * SLOT_MINUTES,
      },
    };
  }

  function buildTimelineItem(id, slotIndex, label, meta, toneKey) {
    return {
      id: id,
      startMinute: slotIndex * SLOT_MINUTES,
      endMinute: (slotIndex + 1) * SLOT_MINUTES,
      label: String(label),
      meta: meta || '',
      tone: toneForKey(toneKey || label),
    };
  }

  function slotRangeLabel(startSlot, endSlot) {
    if (endSlot - startSlot <= 1) {
      return 'Slot ' + String(startSlot + 1);
    }
    return 'Slots ' + String(startSlot + 1) + '-' + String(endSlot);
  }

  function toneForKey(key) {
    var text = String(key || '');
    var hash = 0;

    for (var index = 0; index < text.length; index += 1) {
      hash = ((hash * 31) + text.charCodeAt(index)) >>> 0;
    }

    return TIMELINE_TONES[hash % TIMELINE_TONES.length];
  }

  function containerBadges(length, longestSequence) {
    if (length === 0) return ['Empty'];
    var badges = [];
    if (length === longestSequence) badges.push('Longest');
    if (length === 1) badges.push('Single');
    return badges;
  }

  function describeItem(itemId, itemsById) {
    var item = itemsById[itemId];
    if (!item) {
      return { key: itemId || 'item', name: itemId || 'Unnamed' };
    }
    return {
      key: item.id || itemId || 'item',
      name: item.name || item.id || itemId || 'Unnamed',
    };
  }

  function renderTables(data) {
    tablesContainer.innerHTML = '';

    (config.entities || []).forEach(function (entity) {
      var items = data[entity.plural] || data[entity.name + 's'] || [];
      if (!items.length) return;
      var cols = Object.keys(items[0]);
      var rows = items.map(function (item) {
        return cols.map(function (key) {
          var value = item[key];
          if (value === null || value === undefined) return '—';
          if (Array.isArray(value)) return value.join(', ');
          if (typeof value === 'object') return JSON.stringify(value);
          return String(value);
        });
      });
      var section = SF.el('div', { className: 'sf-section' });
      section.appendChild(SF.el('h3', null, entity.label));
      section.appendChild(SF.createTable({ columns: cols, rows: rows }));
      tablesContainer.appendChild(section);
    });

    (config.facts || []).forEach(function (fact) {
      var items = data[fact.plural] || data[fact.name + 's'] || [];
      if (!items.length) return;
      var cols = Object.keys(items[0]);
      var rows = items.map(function (item) {
        return cols.map(function (key) {
          var value = item[key];
          if (value === null || value === undefined) return '—';
          if (typeof value === 'object') return JSON.stringify(value);
          return String(value);
        });
      });
      var section = SF.el('div', { className: 'sf-section' });
      section.appendChild(SF.el('h3', null, fact.label));
      section.appendChild(SF.createTable({ columns: cols, rows: rows }));
      tablesContainer.appendChild(section);
    });
  }
})();
