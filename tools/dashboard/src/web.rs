// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Eingebettetes Single-Page-Dashboard (HTML + vanilla JS + d3).
//!
//! Wird vom Server als `GET /` ausgeliefert. d3.js wird vom CDN
//! geladen — kein npm-Tooling, keine bundler-Pipeline.

/// Die HTML-Seite mit eingebettetem JS.
pub const INDEX_HTML: &str = r#"<!doctype html>
<html lang="de">
<head>
<meta charset="utf-8">
<title>ZeroDDS Dashboard</title>
<style>
  body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif;
         background: #1a1a1a; color: #ddd; margin: 0; padding: 12px; }
  h1, h2 { color: #fff; }
  h1 { font-size: 22px; margin: 0 0 16px 0; }
  h2 { font-size: 16px; margin: 16px 0 8px 0; border-bottom: 1px solid #444; padding-bottom: 4px; }
  table { width: 100%; border-collapse: collapse; font-size: 13px; }
  th { text-align: left; color: #aaa; font-weight: normal; padding: 4px 8px; border-bottom: 1px solid #333; }
  td { padding: 4px 8px; border-bottom: 1px solid #2a2a2a; font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace; }
  td.num { text-align: right; }
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
  .card { background: #232323; border-radius: 6px; padding: 12px; }
  #graph svg { width: 100%; height: 360px; background: #1f1f1f; border-radius: 4px; }
  #graph .node { fill: #4a9eff; }
  #graph .edge { stroke: #555; stroke-width: 1.5; }
  #graph .label { fill: #ddd; font-size: 11px; }
  button { background: #2a4d7a; color: #fff; border: 0; padding: 6px 12px; border-radius: 4px; cursor: pointer; font-size: 13px; }
  button:hover { background: #356da4; }
  .status-on { color: #5fa05f; font-weight: bold; }
  .status-off { color: #888; }
  .meta { color: #888; font-size: 11px; }
</style>
</head>
<body>
<h1>ZeroDDS Live Dashboard</h1>
<div class="meta">refresh every 1 s — <span id="status">connecting…</span></div>

<div class="grid">
  <div class="card">
    <h2>Topics</h2>
    <table id="topics-tbl">
      <thead><tr><th>name</th><th>type</th><th class="num">pub</th><th class="num">sub</th><th class="num">rate hz</th></tr></thead>
      <tbody></tbody>
    </table>
  </div>
  <div class="card">
    <h2>Participants</h2>
    <table id="participants-tbl">
      <thead><tr><th>name</th><th>guid prefix</th><th class="num">domain</th><th>vendor</th></tr></thead>
      <tbody></tbody>
    </table>
  </div>
</div>

<div class="grid">
  <div class="card">
    <h2>Histograms</h2>
    <table id="hist-tbl">
      <thead><tr><th>metric</th><th class="num">count</th><th class="num">p50</th><th class="num">p99</th><th class="num">max</th></tr></thead>
      <tbody></tbody>
    </table>
  </div>
  <div class="card">
    <h2>Recording</h2>
    <p>Status: <span id="rec-status" class="status-off">off</span></p>
    <p>Path: <span id="rec-path">—</span></p>
    <p>Frames: <span id="rec-frames">0</span></p>
    <button id="rec-toggle">Toggle</button>
  </div>
</div>

<div class="card">
  <h2>Discovery Graph</h2>
  <div id="graph"></div>
</div>

<script src="https://d3js.org/d3.v7.min.js"></script>
<script>
const fmtNs = ns => ns >= 1_000_000 ? (ns/1_000_000).toFixed(2) + ' ms'
                : ns >= 1_000 ? (ns/1_000).toFixed(1) + ' µs'
                : ns + ' ns';

async function refresh() {
  try {
    const [topics, parts, hists, graph, rec] = await Promise.all([
      fetch('/api/topics').then(r => r.json()),
      fetch('/api/participants').then(r => r.json()),
      fetch('/api/histograms').then(r => r.json()),
      fetch('/api/graph').then(r => r.json()),
      fetch('/api/recording').then(r => r.json()),
    ]);
    document.getElementById('status').textContent = 'connected (' + new Date().toLocaleTimeString() + ')';
    renderTopics(topics);
    renderParticipants(parts);
    renderHistograms(hists);
    renderGraph(graph);
    renderRecording(rec);
  } catch (e) {
    document.getElementById('status').textContent = 'error: ' + e.message;
  }
}

function renderTopics(items) {
  const tbody = document.querySelector('#topics-tbl tbody');
  tbody.innerHTML = items.map(t =>
    `<tr><td>${esc(t.name)}</td><td>${esc(t.type_name)}</td>` +
    `<td class="num">${t.publishers}</td><td class="num">${t.subscribers}</td>` +
    `<td class="num">${t.sample_rate_hz.toFixed(1)}</td></tr>`
  ).join('');
}
function renderParticipants(items) {
  const tbody = document.querySelector('#participants-tbl tbody');
  tbody.innerHTML = items.map(p =>
    `<tr><td>${esc(p.name)}</td><td>${esc(p.guid_prefix_hex)}</td>` +
    `<td class="num">${p.domain_id}</td><td>${esc(p.vendor_id_hex)}</td></tr>`
  ).join('');
}
function renderHistograms(items) {
  const tbody = document.querySelector('#hist-tbl tbody');
  tbody.innerHTML = items.map(h =>
    `<tr><td>${esc(h.name)}</td><td class="num">${h.count}</td>` +
    `<td class="num">${fmtNs(h.p50_ns)}</td><td class="num">${fmtNs(h.p99_ns)}</td>` +
    `<td class="num">${fmtNs(h.max_ns)}</td></tr>`
  ).join('');
}
function renderRecording(r) {
  const st = document.getElementById('rec-status');
  st.textContent = r.active ? 'on' : 'off';
  st.className = r.active ? 'status-on' : 'status-off';
  document.getElementById('rec-path').textContent = r.output_path || '—';
  document.getElementById('rec-frames').textContent = r.frames;
}
document.getElementById('rec-toggle').onclick = async () => {
  await fetch('/api/recording/toggle', { method: 'POST' });
  refresh();
};

let graphSim = null;
function renderGraph(g) {
  const div = document.getElementById('graph');
  div.innerHTML = '';
  const svg = d3.select('#graph').append('svg');
  const width = div.clientWidth, height = 360;
  svg.attr('viewBox', `0 0 ${width} ${height}`);

  const link = svg.append('g').selectAll('line').data(g.edges)
    .join('line').attr('class', 'edge');
  const node = svg.append('g').selectAll('circle').data(g.nodes)
    .join('circle').attr('class', 'node').attr('r', 8);
  const labels = svg.append('g').selectAll('text').data(g.nodes)
    .join('text').attr('class', 'label').text(d => d.label).attr('dx', 10).attr('dy', 4);

  graphSim = d3.forceSimulation(g.nodes)
    .force('link', d3.forceLink(g.edges).id(d => d.guid).distance(80))
    .force('charge', d3.forceManyBody().strength(-200))
    .force('center', d3.forceCenter(width / 2, height / 2))
    .on('tick', () => {
      link.attr('x1', d => d.source.x).attr('y1', d => d.source.y)
          .attr('x2', d => d.target.x).attr('y2', d => d.target.y);
      node.attr('cx', d => d.x).attr('cy', d => d.y);
      labels.attr('x', d => d.x).attr('y', d => d.y);
    });
  // Edge-Source/Target sind im JSON GUIDs — d3.forceLink mapped sie via id().
  g.edges.forEach(e => { e.source = e.from_guid; e.target = e.to_guid; });
}
function esc(s) { return String(s).replace(/[<>&"]/g, c => ({'<':'&lt;','>':'&gt;','&':'&amp;','"':'&quot;'}[c])); }

refresh();
setInterval(refresh, 1000);
</script>
</body>
</html>
"#;
