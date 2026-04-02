/* app.js — {{project_name}} SolverForge UI */

(async function () {
  'use strict';

  var config = await fetch('/sf-config.json').then(function (r) { return r.json(); });

  var app = document.getElementById('sf-app');

  // Backend and solver
  var backend = SF.createBackend({ baseUrl: '' });
  var statusBar = SF.createStatusBar({ constraints: config.constraints });
  var solver = SF.createSolver({
    backend: backend,
    statusBar: statusBar,
    onProgress: function (meta) { void meta; },
    onSolution: function (data) { renderSequences(data); renderTables(data); },
    onComplete: function (data) { renderSequences(data); renderTables(data); },
  });

  // Header
  var header = SF.createHeader({
    logo: '/sf/img/ouroboros.svg',
    title: config.title,
    subtitle: config.subtitle,
    tabs: [
      { id: 'sequences', label: 'Sequences', icon: 'fa-list-ol', active: true },
      { id: 'data', label: 'Data', icon: 'fa-table' },
      { id: 'api', label: 'REST API', icon: 'fa-book' },
    ],
    actions: {
      onSolve: function () { loadAndSolve(); },
      onStop: function () { solver.stop(); },
      onAnalyze: function () { openAnalysis(); },
    },
    onTabChange: function (tab) {
      sequencesPanel.style.display = tab === 'sequences' ? '' : 'none';
      dataPanel.style.display = tab === 'data' ? '' : 'none';
      apiPanel.style.display = tab === 'api' ? '' : 'none';
    },
  });
  app.appendChild(header);
  app.appendChild(statusBar.el);

  // Sequences panel (hero)
  var sequencesPanel = SF.el('div', { className: 'sf-content' });
  var sequencesContainer = SF.el('div', { id: 'sf-sequences' });
  sequencesPanel.appendChild(sequencesContainer);
  app.appendChild(sequencesPanel);

  // Data panel
  var dataPanel = SF.el('div', { className: 'sf-content', style: { display: 'none' } });
  var tablesContainer = SF.el('div', { id: 'sf-tables' });
  dataPanel.appendChild(tablesContainer);
  app.appendChild(dataPanel);

  // API panel
  var apiPanel = SF.el('div', { className: 'sf-content', style: { display: 'none' } });
  var guide = SF.createApiGuide({
    endpoints: [
      { method: 'GET', path: '/demo-data/STANDARD', description: 'Fetch demo data', curl: 'curl http://localhost:7860/demo-data/STANDARD' },
      { method: 'POST', path: '/schedules', description: 'Submit a plan for solving', curl: 'curl -X POST -H "Content-Type: application/json" http://localhost:7860/schedules -d @plan.json' },
      { method: 'GET', path: '/schedules/{id}', description: 'Get current best solution', curl: 'curl http://localhost:7860/schedules/{id}' },
      { method: 'GET', path: '/schedules/{id}/events', description: 'Stream solver updates (SSE)', curl: 'curl -N http://localhost:7860/schedules/{id}/events' },
      { method: 'GET', path: '/schedules/{id}/analyze', description: 'Get constraint analysis', curl: 'curl http://localhost:7860/schedules/{id}/analyze' },
      { method: 'DELETE', path: '/schedules/{id}', description: 'Stop solving and remove job', curl: 'curl -X DELETE http://localhost:7860/schedules/{id}' },
    ],
  });
  apiPanel.appendChild(guide);
  app.appendChild(apiPanel);

  // Footer
  var footer = SF.createFooter({
    links: [
      { label: 'SolverForge', url: 'https://www.solverforge.org' },
      { label: 'Docs', url: 'https://www.solverforge.org/docs' },
    ],
  });
  app.appendChild(footer);

  // Analysis modal
  var analysisModal = SF.createModal({ title: 'Score Analysis', width: '700px' });

  // Load demo data on startup
  fetch('/demo-data/STANDARD')
    .then(function (r) { return r.json(); })
    .then(function (data) { renderSequences(data); renderTables(data); })
    .catch(function () {});

  function loadAndSolve() {
    fetch('/demo-data/STANDARD')
      .then(function (r) { return r.json(); })
      .then(function (data) { solver.start(data); })
      .catch(function (err) { console.error('Demo load failed:', err); });
  }

  function openAnalysis() {
    var id = solver.getJobId();
    if (!id) return;
    backend.analyze(id)
      .then(function (analysis) {
        analysisModal.setBody(buildAnalysisHtml(analysis));
        analysisModal.open();
      })
      .catch(function () {});
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

  function renderSequences(data) {
    sequencesContainer.innerHTML = '';
    var containers = data.containers || [];
    if (!containers.length) return;
    var itemsByName = buildItemsByName(data);
    var metrics = deriveSequenceMetrics(containers);
    var sortedContainers = containers.slice().sort(compareContainers);
    var horizon = Math.max(metrics.longestSequence, 1);

    sequencesContainer.appendChild(buildSequenceOverview(metrics));
    sequencesContainer.appendChild(SF.rail.createHeader({
      label: config.entities[0] ? config.entities[0].label : 'Container',
      labelWidth: 220,
      columns: Array.from({ length: horizon }, function (_, i) { return String(i + 1); }),
    }));

    sortedContainers.forEach(function (container) {
      sequencesContainer.appendChild(buildSequenceCard(container, itemsByName, metrics, horizon).el);
    });
  }

  function buildItemsByName(data) {
    var items = data.items || data.itemFacts || data.item_facts || [];
    return items.reduce(function (map, item) {
      if (item && item.name) map[item.name] = item;
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

  function compareContainers(a, b) {
    var aCount = (a.items || []).length;
    var bCount = (b.items || []).length;
    if (bCount !== aCount) return bCount - aCount;
    return String(a.name || '').localeCompare(String(b.name || ''));
  }

  function buildSequenceOverview(metrics) {
    var section = SF.el('div', { className: 'sf-section' });
    section.appendChild(SF.createTable({
      columns: ['Containers', 'Items', 'Longest sequence', 'Empty containers', 'Average items / container'],
      rows: [[
        String(metrics.totalContainers),
        String(metrics.totalItems),
        String(metrics.longestSequence),
        String(metrics.emptyContainers),
        String(metrics.averageItems),
      ]],
    }));
    return section;
  }

  function buildSequenceCard(container, itemsByName, metrics, horizon) {
    var sequence = container.items || [];
    var firstItem = sequence.length ? describeItem(sequence[0], itemsByName).name : '—';
    var lastItem = sequence.length ? describeItem(sequence[sequence.length - 1], itemsByName).name : '—';
    var length = sequence.length;
    var fullnessPct = metrics.longestSequence > 0
      ? Math.round((length / metrics.longestSequence) * 100)
      : 0;
    var card = SF.rail.createCard({
      id: 'container-' + String(container.id != null ? container.id : container.name),
      name: container.name || 'Unnamed container',
      labelWidth: 220,
      columns: horizon,
      type: 'Sequence',
      badges: containerBadges(length, metrics.longestSequence),
      gauges: [
        {
          label: 'Length',
          pct: Math.min(fullnessPct, 100),
          style: length === 0 ? 'heat' : 'load',
          text: String(length) + '/' + String(Math.max(metrics.longestSequence, 1)),
        },
      ],
      stats: [
        { label: 'Items', value: length },
        { label: 'First', value: firstItem },
        { label: 'Last', value: lastItem },
      ],
    });

    sequence.forEach(function (itemName, index) {
      var item = describeItem(itemName, itemsByName);
      card.addBlock({
        id: 'container-' + String(container.id || container.name) + '-item-' + String(index),
        label: item.name,
        meta: 'Pos ' + String(index + 1),
        start: index,
        end: index + 1,
        horizon: horizon,
        color: SF.colors.pick(String(item.key)),
      });
    });

    return card;
  }

  function containerBadges(length, longestSequence) {
    if (length === 0) return ['Empty'];
    var badges = [];
    if (length === longestSequence) badges.push('Longest');
    if (length === 1) badges.push('Single');
    return badges;
  }

  function describeItem(itemName, itemsByName) {
    var item = itemsByName[itemName];
    if (!item) {
      return { key: itemName || 'item', name: itemName || 'Unnamed' };
    }
    return {
      key: item.index != null ? item.index : item.name,
      name: item.name || itemName || 'Unnamed',
    };
  }

  function renderTables(data) {
    tablesContainer.innerHTML = '';

    config.entities.forEach(function (entity) {
      var items = data[entity.plural] || data[entity.name + 's'] || [];
      if (!items.length) return;
      var cols = Object.keys(items[0]);
      var rows = items.map(function (item) {
        return cols.map(function (k) {
          var v = item[k];
          if (v === null || v === undefined) return '—';
          if (Array.isArray(v)) return v.join(', ');
          if (typeof v === 'object') return JSON.stringify(v);
          return String(v);
        });
      });
      var section = SF.el('div', { className: 'sf-section' });
      section.appendChild(SF.el('h3', null, entity.label));
      section.appendChild(SF.createTable({ columns: cols, rows: rows }));
      tablesContainer.appendChild(section);
    });

    config.facts.forEach(function (fact) {
      var items = data[fact.plural] || data[fact.name + 's'] || [];
      if (!items.length) return;
      var cols = Object.keys(items[0]);
      var rows = items.map(function (item) {
        return cols.map(function (k) {
          var v = item[k];
          if (v === null || v === undefined) return '—';
          if (typeof v === 'object') return JSON.stringify(v);
          return String(v);
        });
      });
      var section = SF.el('div', { className: 'sf-section' });
      section.appendChild(SF.el('h3', null, fact.label));
      section.appendChild(SF.createTable({ columns: cols, rows: rows }));
      tablesContainer.appendChild(section);
    });
  }

})();
