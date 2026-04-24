/* app.js — {{project_name}} SolverForge UI */

(async function () {
  'use strict';

  var SLOT_MINUTES = 60;
  var DEFAULT_VIEWPORT_SLOTS = 12;
  var TIMELINE_TONES = ['emerald', 'blue', 'amber', 'rose', 'violet', 'slate'];

  var config = await fetch('/sf-config.json').then(function (response) { return response.json(); });
  var uiModel = await fetch('/generated/ui-model.json').then(function (response) { return response.json(); });
  var app = document.getElementById('sf-app');
  var backend = SF.createBackend({ baseUrl: '' });
  var statusBar = SF.createStatusBar({ constraints: uiModel.constraints || [] });
  var currentPlan = null;
  var lastAnalysis = null;
  var bootstrapError = null;
  var demoCatalog = { defaultId: null, availableIds: [] };
  var activeTab = (uiModel.views && uiModel.views.length) ? uiModel.views[0].id : 'overview';
  var viewPanels = {};
  var viewTimelines = {};

  var tabs = (uiModel.views || []).map(function (view, index) {
    return {
      id: view.id,
      label: view.label,
      icon: view.kind === 'list' ? 'fa-list-ol' : 'fa-table-cells-large',
      active: index === 0,
    };
  });
  if (!tabs.length) {
    tabs.push({ id: 'overview', label: 'Overview', icon: 'fa-compass', active: true });
  }
  tabs.push({ id: 'data', label: 'Data', icon: 'fa-table' });
  tabs.push({ id: 'api', label: 'REST API', icon: 'fa-book' });

  var header = SF.createHeader({
    logo: '/sf/img/ouroboros.svg',
    title: config.title,
    subtitle: config.subtitle,
    tabs: tabs,
    actions: {
      onSolve: function () { loadAndSolve(); },
      onPause: function () { pauseSolve(); },
      onResume: function () { resumeSolve(); },
      onCancel: function () { cancelSolve(); },
      onAnalyze: function () { openAnalysis(); },
    },
    onTabChange: function (tab) {
      activeTab = tab;
      Object.keys(viewPanels).forEach(function (key) {
        viewPanels[key].style.display = key === tab ? '' : 'none';
      });
      overviewPanel.style.display = tab === 'overview' ? '' : 'none';
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

  var overviewPanel = SF.el('div', { className: 'sf-content', style: { display: activeTab === 'overview' ? '' : 'none' } });
  var overviewContainer = SF.el('div', { id: 'sf-overview' });
  overviewPanel.appendChild(overviewContainer);
  app.appendChild(overviewPanel);

  (uiModel.views || []).forEach(function (view) {
    var panel = SF.el('div', { className: 'sf-content', style: { display: activeTab === view.id ? '' : 'none' } });
    panel.appendChild(SF.el('div', { id: 'view-' + view.id }));
    viewPanels[view.id] = panel;
    app.appendChild(panel);
  });

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
      if (snapshot && snapshot.solution) {
        renderAll(snapshot.solution);
      }
      syncLifecycleMarkers(meta);
    },
    onPaused: function (snapshot, meta) {
      if (snapshot && snapshot.solution) {
        renderAll(snapshot.solution);
      }
      syncLifecycleMarkers(meta);
    },
    onResumed: function (meta) {
      syncLifecycleMarkers(meta);
    },
    onCancelled: function (snapshot, meta) {
      if (snapshot && snapshot.solution) {
        renderAll(snapshot.solution);
      }
      syncLifecycleMarkers(meta);
    },
    onComplete: function (snapshot, meta) {
      if (snapshot && snapshot.solution) {
        renderAll(snapshot.solution);
      }
      syncLifecycleMarkers(meta);
    },
    onFailure: function (message, meta, snapshot, analysis) {
      if (snapshot && snapshot.solution) {
        renderAll(snapshot.solution);
      }
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
  renderApiGuide();
  updateSolveActionAvailability();
  bootstrapDemoData();

  window.addEventListener('beforeunload', destroyAllTimelines);

  function loadAndSolve() {
    if (solver.isRunning() || solver.getLifecycleState() === 'PAUSED' || !canSolve()) return;
    cleanupTerminalJob()
      .then(function (data) {
        return data || resolvePlanForSolve();
      })
      .then(function (data) {
        return solver.start(data);
      })
      .then(function () {
        syncLifecycleMarkers();
      })
      .catch(function (err) { console.error('Solve start failed:', err); });
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

  function openAnalysis() {
    if (!solver.getJobId()) return;
    solver.analyzeSnapshot()
      .then(function (analysis) {
        lastAnalysis = analysis;
        analysisModal.setBody(buildAnalysisHtml(analysis));
        analysisModal.open();
      })
      .catch(function () {});
  }

  function renderAll(data) {
    currentPlan = clonePlan(data);
    renderOverview(data);
    renderViews(data);
    renderTables(data);
  }

  function resolvePlanForSolve() {
    if (currentPlan) {
      return Promise.resolve(clonePlan(currentPlan));
    }
    if (!demoCatalog.defaultId) {
      return Promise.reject(new Error('demo data catalog is unavailable'));
    }
    return fetchDemoPlan(demoCatalog.defaultId);
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
      { method: 'GET', path: '/jobs/{id}', description: 'Get current job summary', curl: buildCurlCommand('GET', '/jobs/{id}') },
      { method: 'GET', path: '/jobs/{id}/snapshot', description: 'Fetch the latest retained snapshot', curl: buildCurlCommand('GET', '/jobs/{id}/snapshot') },
      { method: 'GET', path: '/jobs/{id}/analysis?snapshot_revision={n}', description: 'Analyze an exact snapshot revision', curl: buildCurlCommand('GET', '/jobs/{id}/analysis?snapshot_revision=3', { quoteUrl: true }) },
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

  function cleanupTerminalJob() {
    var state = solver.getLifecycleState();
    if (!solver.getJobId() || state === 'IDLE' || state === 'PAUSED' || solver.isRunning()) {
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

  function clonePlan(data) {
    return JSON.parse(JSON.stringify(data));
  }

  function renderOverview(data) {
    overviewContainer.innerHTML = '';
    if ((uiModel.views || []).length) {
      overviewContainer.appendChild(SF.el(
        'p',
        null,
        'The generated views now mount the canonical solverforge-ui timeline surface for every planning variable declared in your project.'
      ));
      overviewContainer.appendChild(SF.createTable({
        columns: ['Active views', 'Constraints', 'Current score'],
        rows: [[
          String(uiModel.views.length),
          String((uiModel.constraints || []).length),
          String(data.score || '—'),
        ]],
      }));
      return;
    }
    overviewContainer.appendChild(SF.el('p', null, 'No planning variables are declared yet. Use `solverforge generate entity`, `generate fact`, and `generate variable` to shape the app.'));
  }

  function renderViews(data) {
    (uiModel.views || []).forEach(function (view) {
      var container = document.getElementById('view-' + view.id);
      if (!container) return;
      if (view.kind === 'list') {
        renderTimelinePanel(
          container,
          view.id,
          buildListViewPayload(data, view),
          'This list-variable timeline will appear once the referenced facts and entities contain data.'
        );
      } else {
        renderTimelinePanel(
          container,
          view.id,
          buildScalarViewPayload(data, view),
          'This scalar-variable timeline will appear once the referenced facts and entities contain data.'
        );
      }
    });
  }

  function renderTimelinePanel(container, viewId, payload, emptyMessage) {
    container.innerHTML = '';
    if (!payload) {
      destroyTimeline(viewId);
      container.appendChild(SF.el('p', null, emptyMessage));
      return;
    }

    container.appendChild(payload.summary);
    container.appendChild(ensureTimeline(viewId, payload.timeline).el);
  }

  function ensureTimeline(viewId, timelineConfig) {
    var timeline = viewTimelines[viewId];
    if (!timeline) {
      timeline = SF.rail.createTimeline(timelineConfig);
      viewTimelines[viewId] = timeline;
      return timeline;
    }

    timeline.setModel(timelineConfig.model);
    return timeline;
  }

  function destroyTimeline(viewId) {
    var timeline = viewTimelines[viewId];
    if (!timeline) return;
    timeline.destroy();
    delete viewTimelines[viewId];
  }

  function destroyAllTimelines() {
    Object.keys(viewTimelines).forEach(function (viewId) {
      destroyTimeline(viewId);
    });
  }

  function buildScalarViewPayload(data, view) {
    var entities = data[view.entityPlural] || [];
    var facts = data[view.sourcePlural] || [];
    if (!entities.length || !facts.length) return null;

    var byIndex = {};
    facts.forEach(function (fact, index) {
      byIndex[index] = fact;
    });

    var assignments = facts.map(function () { return []; });
    var detached = [];
    entities.forEach(function (entity) {
      var idx = entity[view.variableField];
      if (idx == null || byIndex[idx] == null) {
        detached.push(entity);
        return;
      }
      assignments[idx].push(entity);
    });

    var peakLoad = assignments.reduce(function (maxCount, items) {
      return Math.max(maxCount, items.length);
    }, 0);
    var horizon = Math.max(peakLoad, detached.length, 1);
    var axis = buildSlotAxis(horizon);
    var lanes = facts.map(function (fact, factIndex) {
      var items = assignments[factIndex] || [];
      return {
        id: view.id + '-lane-' + factIndex,
        label: String(factLabel(fact, factIndex)),
        mode: 'detailed',
        badges: items.length ? [] : ['Empty'],
        stats: [{ label: title(view.entityPlural), value: items.length }],
        items: items.map(function (entity, itemIndex) {
          return buildTimelineItem(
            view.id + '-fact-' + factIndex + '-entity-' + itemIndex,
            itemIndex,
            entityLabel(entity, itemIndex),
            'Assignment ' + String(itemIndex + 1),
            entityLabel(entity, itemIndex)
          );
        }),
      };
    });

    if (detached.length) {
      lanes.push({
        id: view.id + '-detached',
        label: view.allowsUnassigned ? 'Unassigned' : 'Unmapped',
        mode: 'detailed',
        badges: [view.allowsUnassigned ? 'Needs assignment' : 'Out of range'],
        stats: [{ label: title(view.entityPlural), value: detached.length }],
        items: detached.map(function (entity, itemIndex) {
          return buildTimelineItem(
            view.id + '-detached-' + itemIndex,
            itemIndex,
            entityLabel(entity, itemIndex),
            view.allowsUnassigned ? 'Awaiting assignment' : 'Invalid source index',
            entityLabel(entity, itemIndex)
          );
        }),
      });
    }

    return {
      summary: buildSummarySection(
        ['Source lanes', title(view.entityPlural), 'Peak load', 'Unassigned'],
        [
          String(facts.length),
          String(entities.length),
          String(peakLoad),
          String(detached.length),
        ]
      ),
      timeline: {
        label: title(view.sourcePlural),
        labelWidth: 280,
        title: view.label,
        subtitle: title(view.entityPlural) + ' grouped by ' + title(view.sourcePlural),
        model: {
          axis: axis,
          lanes: lanes,
        },
      },
    };
  }

  function buildListViewPayload(data, view) {
    var entities = data[view.entityPlural] || [];
    var facts = data[view.sourcePlural] || [];
    if (!entities.length || !facts.length) return null;

    var byIndex = {};
    facts.forEach(function (fact, index) {
      byIndex[index] = fact;
    });

    var rows = entities.map(function (entity, entityIndex) {
      var sequence = Array.isArray(entity[view.variableField]) ? entity[view.variableField] : [];
      return {
        entity: entity,
        entityIndex: entityIndex,
        sequence: sequence,
      };
    });

    rows.sort(function (left, right) {
      if (right.sequence.length !== left.sequence.length) {
        return right.sequence.length - left.sequence.length;
      }
      return String(entityLabel(left.entity, left.entityIndex)).localeCompare(
        String(entityLabel(right.entity, right.entityIndex))
      );
    });

    var totalItems = rows.reduce(function (sum, row) {
      return sum + row.sequence.length;
    }, 0);
    var longestSequence = rows.reduce(function (maxCount, row) {
      return Math.max(maxCount, row.sequence.length);
    }, 0);
    var emptyEntities = rows.filter(function (row) { return row.sequence.length === 0; }).length;
    var horizon = Math.max(longestSequence, 1);
    var axis = buildSlotAxis(horizon);

    var lanes = rows.map(function (row) {
      return {
        id: view.id + '-entity-' + row.entityIndex,
        label: entityLabel(row.entity, row.entityIndex),
        mode: 'detailed',
        badges: listLaneBadges(row.sequence.length, longestSequence),
        stats: [{ label: title(view.sourcePlural), value: row.sequence.length }],
        items: row.sequence.map(function (factIndex, sequenceIndex) {
          var fact = byIndex[factIndex];
          return buildTimelineItem(
            view.id + '-entity-' + row.entityIndex + '-item-' + sequenceIndex,
            sequenceIndex,
            factLabel(fact, factIndex),
            'Position ' + String(sequenceIndex + 1),
            factLabel(fact, factIndex)
          );
        }),
      };
    });

    return {
      summary: buildSummarySection(
        [title(view.entityPlural), title(view.sourcePlural), 'Longest sequence', 'Empty lanes', 'Average items / lane'],
        [
          String(rows.length),
          String(totalItems),
          String(longestSequence),
          String(emptyEntities),
          rows.length ? (totalItems / rows.length).toFixed(1) : '0.0',
        ]
      ),
      timeline: {
        label: title(view.entityPlural),
        labelWidth: 280,
        title: view.label,
        subtitle: title(view.sourcePlural) + ' ordered inside each ' + title(view.entityPlural),
        model: {
          axis: axis,
          lanes: lanes,
        },
      },
    };
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

  function listLaneBadges(length, longestSequence) {
    if (length === 0) return ['Empty'];
    var badges = [];
    if (length === longestSequence) badges.push('Longest');
    if (length === 1) badges.push('Single');
    return badges;
  }

  function toneForKey(key) {
    var text = String(key || '');
    var hash = 0;

    for (var index = 0; index < text.length; index += 1) {
      hash = ((hash * 31) + text.charCodeAt(index)) >>> 0;
    }

    return TIMELINE_TONES[hash % TIMELINE_TONES.length];
  }

  function renderTables(data) {
    tablesContainer.innerHTML = '';
    (uiModel.entities || []).concat(uiModel.facts || []).forEach(function (entry) {
      var rows = data[entry.plural] || [];
      if (!rows.length) return;
      var cols = Object.keys(rows[0]).filter(function (key) { return key !== 'score' && key !== 'solverStatus'; });
      var values = rows.map(function (row) {
        return cols.map(function (key) {
          var value = row[key];
          if (value == null) return '—';
          if (Array.isArray(value)) return value.join(', ');
          if (typeof value === 'object') return JSON.stringify(value);
          return String(value);
        });
      });
      var section = SF.el('div', { className: 'sf-section' });
      section.appendChild(SF.el('h3', null, entry.label));
      section.appendChild(SF.createTable({ columns: cols, rows: values }));
      tablesContainer.appendChild(section);
    });
  }

  function buildAnalysisHtml(analysis) {
    if (!analysis || !analysis.constraints) return '<p>No analysis available.</p>';
    var html = '<p><strong>Score:</strong> ' + SF.escHtml(analysis.score) + '</p>';
    html += '<table class="sf-table"><thead><tr><th>Constraint</th><th>Type</th><th>Score</th><th>Matches</th></tr></thead><tbody>';
    analysis.constraints.forEach(function (constraint) {
      var matchCount = constraint.matchCount != null ? constraint.matchCount : (constraint.matches ? constraint.matches.length : 0);
      html += '<tr><td>' + SF.escHtml(constraint.name) + '</td><td>' + SF.escHtml(constraint.constraintType || constraint.type || '') + '</td><td>' + SF.escHtml(constraint.score) + '</td><td>' + matchCount + '</td></tr>';
    });
    html += '</tbody></table>';
    return html;
  }

  function factLabel(fact, fallback) {
    if (!fact) return String(fallback);
    return fact.name || fact.id || fallback;
  }

  function entityLabel(entity, fallback) {
    if (!entity) return String(fallback);
    return entity.name || entity.id || fallback;
  }

  function title(text) {
    return String(text || '')
      .replace(/_/g, ' ')
      .replace(/\b\w/g, function (match) { return match.toUpperCase(); });
  }
})();
