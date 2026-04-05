/* app.js — {{project_name}} SolverForge UI */

(async function () {
  'use strict';

  var config = await fetch('/sf-config.json').then(function (r) { return r.json(); });
  var uiModel = await fetch('/generated/ui-model.json').then(function (r) { return r.json(); });
  var app = document.getElementById('sf-app');
  var backend = SF.createBackend({ baseUrl: '' });
  var statusBar = SF.createStatusBar({ constraints: uiModel.constraints || [] });
  var currentPlan = null;
  var currentScheduleId = null;
  var solvingJobId = null;
  var closeStream = null;
  var solveRunToken = 0;
  var activeTab = (uiModel.views && uiModel.views.length) ? uiModel.views[0].id : 'overview';
  var viewPanels = {};

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
      onStop: function () { stopSolve(); },
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
  apiPanel.appendChild(SF.createApiGuide({
    endpoints: [
      { method: 'GET', path: '/demo-data/STANDARD', description: 'Fetch demo data', curl: 'curl http://localhost:7860/demo-data/STANDARD' },
      { method: 'POST', path: '/schedules', description: 'Submit a plan for solving', curl: 'curl -X POST -H "Content-Type: application/json" http://localhost:7860/schedules -d @plan.json' },
      { method: 'GET', path: '/schedules/{id}', description: 'Get current best solution', curl: 'curl http://localhost:7860/schedules/{id}' },
      { method: 'GET', path: '/schedules/{id}/events', description: 'Stream solver updates (SSE)', curl: 'curl -N http://localhost:7860/schedules/{id}/events' },
      { method: 'GET', path: '/schedules/{id}/analyze', description: 'Get constraint analysis', curl: 'curl http://localhost:7860/schedules/{id}/analyze' },
      { method: 'DELETE', path: '/schedules/{id}', description: 'Stop solving and keep the latest checkpoint for resume', curl: 'curl -X DELETE http://localhost:7860/schedules/{id}' },
    ],
  }));
  app.appendChild(apiPanel);

  app.appendChild(SF.createFooter({
    links: [
      { label: 'SolverForge', url: 'https://www.solverforge.org' },
      { label: 'Docs', url: 'https://www.solverforge.org/docs' },
    ],
  }));

  var analysisModal = SF.createModal({ title: 'Score Analysis', width: '700px' });

  fetch('/demo-data/STANDARD')
    .then(function (r) { return r.json(); })
    .then(function (data) { renderAll(data); })
    .catch(function () {});

  function loadAndSolve() {
    if (solvingJobId) return;
    solveRunToken += 1;
    var token = solveRunToken;
    resolvePlanForSolve()
      .then(function (data) {
        if (token !== solveRunToken) return;
        startSolve(data, token);
      })
      .catch(function (err) { console.error('Solve start failed:', err); });
  }

  function stopSolve() {
    if (!solvingJobId) return;
    backend.deleteSchedule(solvingJobId)
      .catch(function (err) { console.error('Stop failed:', err); });
  }

  function openAnalysis() {
    var id = currentScheduleId;
    if (!id) return;
    backend.analyze(id)
      .then(function (analysis) {
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
    return fetch('/demo-data/STANDARD')
      .then(function (r) { return r.json(); });
  }

  function startSolve(data, token) {
    statusBar.setSolving(true);
    statusBar.updateMoves(null);
    backend.createSchedule(data)
      .then(function (id) {
        if (token !== solveRunToken || !id) return;
        currentScheduleId = id;
        solvingJobId = id;
        closeExistingStream();
        closeStream = backend.streamEvents(id, function (msg) {
          if (token !== solveRunToken || !msg || typeof msg !== 'object') return;
          handleSolverEvent(msg, id, token);
        }, function (err) {
          if (token !== solveRunToken) return;
          console.error('Solver event stream failed:', err);
          finishSolve(id, false);
        });
      })
      .catch(function (err) {
        if (token !== solveRunToken) return;
        console.error('Create schedule failed:', err);
        finishSolve(null, false);
      });
  }

  function handleSolverEvent(msg, id, token) {
    if (token !== solveRunToken) return;
    if (msg.id && String(msg.id) !== String(id)) return;
    if (!msg.eventType) return;

    statusBar.updateScore(msg.currentScore || null);
    statusBar.updateMoves(msg.movesPerSecond || null);

    if (msg.eventType === 'best_solution' || msg.eventType === 'finished') {
      if (msg.solution) {
        currentScheduleId = id;
        renderAll(msg.solution);
      }
    }

    if (msg.eventType === 'finished') {
      finishSolve(id, true);
    }
  }

  function finishSolve(id, keepSchedule) {
    closeExistingStream();
    solvingJobId = null;
    if (!keepSchedule) {
      currentScheduleId = id || currentScheduleId;
    }
    statusBar.setSolving(false);
    statusBar.updateMoves(null);
  }

  function closeExistingStream() {
    if (closeStream) {
      closeStream();
      closeStream = null;
    }
  }

  function clonePlan(data) {
    return JSON.parse(JSON.stringify(data));
  }

  function renderOverview(data) {
    overviewContainer.innerHTML = '';
    if ((uiModel.views || []).length) {
      overviewContainer.appendChild(SF.el('p', null, 'Add facts, entities, and planning variables in any order. The view tabs are generated from the variables declared in your project.'));
      overviewContainer.appendChild(SF.createTable({
        columns: ['Active views', 'Constraints', 'Current score'],
        rows: [[String(uiModel.views.length), String((uiModel.constraints || []).length), String(data.score || '—')]],
      }));
      return;
    }
    overviewContainer.appendChild(SF.el('p', null, 'No planning variables are declared yet. Use `solverforge generate entity`, `generate fact`, and `generate variable` to shape the app.'));
  }

  function renderViews(data) {
    (uiModel.views || []).forEach(function (view) {
      var container = document.getElementById('view-' + view.id);
      if (!container) return;
      container.innerHTML = '';
      if (view.kind === 'list') {
        renderListView(container, data, view);
      } else {
        renderStandardView(container, data, view);
      }
    });
  }

  function renderStandardView(container, data, view) {
    var entities = data[view.entityPlural] || [];
    var facts = data[view.sourcePlural] || [];
    if (!entities.length || !facts.length) {
      container.appendChild(SF.el('p', null, 'This standard-variable view will appear once the referenced facts and entities contain data.'));
      return;
    }

    var byIndex = {};
    facts.forEach(function (fact, index) {
      byIndex[index] = fact;
    });

    var assignments = {};
    facts.forEach(function (fact, index) {
      assignments[String(factLabel(fact, index))] = [];
    });
    var unassigned = [];
    entities.forEach(function (entity) {
      var idx = entity[view.variableField];
      if (idx == null || byIndex[idx] == null) {
        unassigned.push(entity);
        return;
      }
      assignments[String(factLabel(byIndex[idx], idx))].push(entity);
    });

    var horizon = Math.max(maxColumns(assignments), unassigned.length, 1);
    container.appendChild(SF.rail.createHeader({
      label: title(view.sourcePlural),
      labelWidth: 220,
      columns: Array.from({ length: horizon }, function (_, i) { return 'Slot ' + (i + 1); }),
    }));

    facts.forEach(function (fact, index) {
      var label = String(factLabel(fact, index));
      var items = assignments[label] || [];
      var card = SF.rail.createCard({
        id: view.id + '-fact-' + index,
        name: label,
        labelWidth: 220,
        columns: horizon,
        stats: [{ label: title(view.entityPlural), value: items.length }],
      });
      items.forEach(function (entity, itemIndex) {
        card.addBlock({
          id: view.id + '-entity-' + itemIndex,
          label: entityLabel(entity, itemIndex),
          start: itemIndex,
          end: itemIndex + 1,
          horizon: horizon,
          color: SF.colors.pick(String(entityLabel(entity, itemIndex))),
        });
      });
      container.appendChild(card.el);
    });

    if (unassigned.length && view.allowsUnassigned) {
      var unassignedCard = SF.rail.createCard({
        id: view.id + '-unassigned',
        name: 'Unassigned',
        labelWidth: 220,
        columns: horizon,
        badges: ['Needs assignment'],
        stats: [{ label: title(view.entityPlural), value: unassigned.length }],
      });
      unassigned.forEach(function (entity, itemIndex) {
        unassignedCard.addBlock({
          id: view.id + '-unassigned-' + itemIndex,
          label: entityLabel(entity, itemIndex),
          start: itemIndex,
          end: itemIndex + 1,
          horizon: horizon,
          color: SF.colors.pick(String(entityLabel(entity, itemIndex))),
        });
      });
      container.appendChild(unassignedCard.el);
    }
  }

  function renderListView(container, data, view) {
    var entities = data[view.entityPlural] || [];
    var facts = data[view.sourcePlural] || [];
    if (!entities.length || !facts.length) {
      container.appendChild(SF.el('p', null, 'This list-variable view will appear once the referenced facts and entities contain data.'));
      return;
    }
    var byIndex = {};
    facts.forEach(function (fact, index) {
      byIndex[index] = fact;
    });
    var horizon = entities.reduce(function (max, entity) {
      var list = entity[view.variableField] || [];
      return Math.max(max, list.length);
    }, 1);

    container.appendChild(SF.rail.createHeader({
      label: title(view.entityPlural),
      labelWidth: 220,
      columns: Array.from({ length: horizon }, function (_, i) { return String(i + 1); }),
    }));

    entities.forEach(function (entity, entityIndex) {
      var sequence = entity[view.variableField] || [];
      var card = SF.rail.createCard({
        id: view.id + '-entity-' + entityIndex,
        name: entityLabel(entity, entityIndex),
        labelWidth: 220,
        columns: horizon,
        stats: [{ label: title(view.sourcePlural), value: sequence.length }],
      });
      sequence.forEach(function (itemIndex, seqIndex) {
        var fact = byIndex[itemIndex];
        card.addBlock({
          id: view.id + '-item-' + seqIndex,
          label: factLabel(fact, itemIndex),
          start: seqIndex,
          end: seqIndex + 1,
          horizon: horizon,
          color: SF.colors.pick(String(factLabel(fact, itemIndex))),
        });
      });
      container.appendChild(card.el);
    });
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
    analysis.constraints.forEach(function (c) {
      html += '<tr><td>' + SF.escHtml(c.name) + '</td><td>' + SF.escHtml(c.constraintType || c.type || '') + '</td><td>' + SF.escHtml(c.score) + '</td><td>' + (c.matches ? c.matches.length : 0) + '</td></tr>';
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

  function maxColumns(assignments) {
    return Object.keys(assignments).reduce(function (max, key) {
      return Math.max(max, (assignments[key] || []).length);
    }, 0);
  }

  function title(text) {
    return String(text || '')
      .replace(/_/g, ' ')
      .replace(/\b\w/g, function (match) { return match.toUpperCase(); });
  }
})();
