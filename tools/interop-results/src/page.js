// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Inlined into matrix.html. No import, no fetch, no external resource:
// the appendix has to work from a file:// URL with the network off.
//
// The appendix is paginated rather than put in the DOM whole. Fifty
// rows of thirteen columns is a table a browser lays out in a frame;
// 3,350 of them is not, and a row that expands to a detail panel makes
// every row a different height, which is what makes virtualisation the
// harder of the two options here rather than the easier one.

(function () {
  "use strict";

  var node = document.getElementById("matrix");
  if (!node) return;
  var DATA = JSON.parse(node.textContent);
  var S = DATA.strings;
  var SYS = DATA.systems;
  var CASES = DATA.cases;
  var MODES = DATA.modes;
  var ROWS = DATA.rows;
  var SETTINGS = DATA.settings;
  var BASE = DATA.base || "";
  var PAGE = 50;

  var body = document.getElementById("body");
  var pager = document.getElementById("pager");
  var count = document.getElementById("count");
  var query = document.getElementById("q");
  var chips = document.getElementById("chips");

  var filtered = ROWS;
  var page = 0;
  var picked = null;
  var open = Object.create(null);
  var blobs = null;
  var byId = Object.create(null);
  var byCase = Object.create(null);
  for (var i = 0; i < ROWS.length; i++) byId[ROWS[i][0]] = i;
  for (var c = 0; c < CASES.length; c++) byCase[CASES[c].n] = c;

  // Which words mean "this system was never asked". They are printed
  // rather than left blank, and they are greyed rather than coloured,
  // because an absence is not a result.
  var VOID = {
    "no xcdr2": 1,
    "one order": 1,
    "no form": 1,
    "no probe": 1,
    "not measured": 1,
    unwritten: 1,
    "=": 1,
  };

  function esc(text) {
    return String(text)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  // The two Markdown forms the generated sentences carry.
  function inline(text) {
    var out = esc(text);
    out = out.replace(/`([^`]*)`/g, "<code>$1</code>");
    out = out.replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>");
    return out;
  }

  function modeOf(row) {
    return MODES[row[2]];
  }

  function encodingOf(row) {
    return modeOf(row).split("/")[0];
  }

  function orderOf(row) {
    return modeOf(row).split("/")[1];
  }

  function cell(row, at) {
    var base = at * 7;
    var cells = row[7];
    return {
      write: S[cells[base]],
      writeDetail: S[cells[base + 1]],
      read: S[cells[base + 2]],
      readDetail: S[cells[base + 3]],
      warnings: cells[base + 4],
      agreement: cells[base + 5],
      // One bit per setting: which of the ten reproduce this system's
      // octets on this row. Zero on a row the system wrote means no
      // setting reaches it; zero on a row it did not write means
      // nothing was asked.
      reproduce: cells[base + 6],
    };
  }

  function path(row, system, tail) {
    return BASE + CASES[row[1]].n + "/" + system.id + "/" + tail;
  }

  function cellLink(row, system, value) {
    if (value.write === "octets" || value.write === "bytes") {
      return {
        href: path(row, system, encodingOf(row) + "." + orderOf(row) + ".bin"),
        label: value.writeDetail.length / 2 + "B",
      };
    }
    if (value.write === "=") {
      return {
        href: path(row, system, value.writeDetail + "." + orderOf(row) + ".bin"),
        label: "= " + value.writeDetail,
      };
    }
    return { href: path(row, system, "STATUS"), label: value.write };
  }

  // The searchable text of a row: everything printed on it, plus the
  // symptom words a reporter would use. A reporter searching for a
  // symptom is not searching for a case name, and the two have to be
  // askable apart.
  //
  // A system and a symptom are one token joined by a colon —
  // `cyclone:differs` — rather than two words. Two words would be
  // matched independently, and since almost every row names almost
  // every system somewhere (in the diff sentence, in a refusal, in a
  // structural word), "cyclone differs" would answer with every row
  // where anything differs. The colon form asks the question that was
  // meant. The loose form still works and still means the loose thing.
  function blobOf(at) {
    if (blobs === null) blobs = new Array(ROWS.length);
    if (blobs[at] !== undefined) return blobs[at];
    var row = ROWS[at];
    var kase = CASES[row[1]];
    var parts = [
      "row " + row[0],
      kase.n,
      kase.d,
      modeOf(row),
      encodingOf(row),
      orderOf(row) === "le" ? "le little endian" : "be big endian",
      S[row[3]],
      S[row[4]],
    ];
    for (var k = 0; k < kase.i.length; k++) parts.push("#" + kase.i[k]);
    for (var s = 0; s < SYS.length; s++) {
      var value = cell(row, s);
      var id = SYS[s].id;
      var wrote = value.write === "octets" || value.write === "bytes";
      if (value.agreement === 2) parts.push(id + ":differs differs");
      if (value.write === "refused")
        parts.push(id + ":refused refused " + value.writeDetail);
      if (value.read === "different") parts.push(id + ":misread misread");
      if (value.warnings > 0) parts.push(id + ":warned warned");
      if (VOID[value.write])
        parts.push(id + ":" + value.write.replace(/[ =]/g, "-"));
      // The remainder, asked per setting: every flag that does *not*
      // produce what this system produced here. This is the token the
      // dialect matrix links with, so a cell in that table and the rows
      // it opens are the same measurement read twice rather than two
      // measurements.
      if (wrote) {
        for (var f = 0; f < SETTINGS.length; f++) {
          if ((value.reproduce & (1 << f)) === 0)
            parts.push(id + ":unmatched-" + SETTINGS[f]);
        }
        if (value.reproduce === 0)
          parts.push(id + ":unalignable unalignable no setting reaches it");
      }
    }
    blobs[at] = parts.join(" · ").toLowerCase();
    return blobs[at];
  }

  function applyFilter(text) {
    var terms = String(text || "")
      .toLowerCase()
      .split(/\s+/)
      .filter(function (term) {
        return term.length > 0;
      });
    if (terms.length === 0) {
      filtered = ROWS;
      return;
    }
    // A term naming a system is anchored at the start of its token.
    // `ros-cyclone:differs` ends with `cyclone:differs`, and a reader
    // who asked about Cyclone and was given rows about the ROS type
    // support would have been answered a question they did not ask.
    // Every token in a blob is preceded by a space, so a leading space
    // is the whole of the anchor.
    var needles = terms.map(function (term) {
      return term.indexOf(":") === -1 ? term : " " + term;
    });
    var out = [];
    for (var at = 0; at < ROWS.length; at++) {
      var blob = blobOf(at);
      var every = true;
      for (var t = 0; t < needles.length; t++) {
        if (blob.indexOf(needles[t]) === -1) {
          every = false;
          break;
        }
      }
      if (every) out.push(ROWS[at]);
    }
    filtered = out;
  }

  function renderCount() {
    var total = ROWS.length;
    var shown = filtered.length;
    if (shown === total) {
      count.textContent = total + " rows";
    } else {
      count.textContent = shown + " of " + total + " rows match";
    }
  }

  function renderRow(row) {
    var kase = CASES[row[1]];
    var html =
      '<tr id="row-' +
      row[0] +
      '" class="row' +
      (picked === row[0] ? " picked" : "") +
      (open[row[0]] ? " open" : "") +
      '" data-id="' +
      row[0] +
      '">';
    html +=
      '<td class="n num"><a href="#row-' +
      row[0] +
      '" title="link to this row">' +
      row[0] +
      "</a></td>";
    html +=
      '<td class="construct"><button type="button" class="expand" data-id="' +
      row[0] +
      '" aria-expanded="' +
      (open[row[0]] ? "true" : "false") +
      '"><span class="name">' +
      esc(kase.n) +
      "</span></button></td>";
    html += '<td class="mode">' + esc(modeOf(row)) + "</td>";
    for (var s = 0; s < SYS.length; s++) {
      var value = cell(row, s);
      var link = cellLink(row, SYS[s], value);
      var klass = "cell ";
      if (value.agreement === 1) klass += "w-agree";
      else if (value.agreement === 2) klass += "w-differ";
      else if (value.write === "refused") klass += "w-refused";
      else if (VOID[value.write]) klass += "w-void";
      else klass += "w-octets";
      html +=
        '<td class="' +
        klass +
        '"><a href="' +
        esc(link.href) +
        '" title="' +
        esc(SYS[s].label + " — " + value.write) +
        '">' +
        esc(link.label) +
        "</a>";
      if (value.warnings > 0) {
        html +=
          '<a class="warn-mark" href="' +
          esc(path(row, SYS[s], "WARNINGS")) +
          '" title="its compiler said ' +
          value.warnings +
          ' thing(s) about this type">w' +
          value.warnings +
          "</a>";
      }
      html += "</td>";
    }
    html += "<td>";
    for (var k = 0; k < kase.i.length; k++) {
      html +=
        '<a href="#issue-' + kase.i[k] + '">#' + kase.i[k] + "</a> ";
    }
    html += "</td></tr>";
    if (open[row[0]]) html += renderDetail(row);
    return html;
  }

  function renderDetail(row) {
    var kase = CASES[row[1]];
    var diff = S[row[3]];
    var mitigation = S[row[4]];
    var ours = S[row[5]];
    var spec = S[row[6]];
    var html =
      '<tr class="detail"><td colspan="' +
      (SYS.length + 4) +
      '"><div class="detail-body">';
    html += "<p>" + inline(kase.d) + "</p>";
    html +=
      '<p><a href="' +
      esc(BASE + "../interop-corpus/" + kase.n + "/type.idl") +
      '">the IDL</a> · <a href="' +
      esc(BASE + "../interop-corpus/" + kase.n + "/values.json") +
      '">the value</a> · mode <code>' +
      esc(modeOf(row)) +
      "</code></p>";

    if (diff) {
      html += "<div><h5>What differs</h5><p>" + inline(diff) + "</p></div>";
    } else {
      html +=
        "<div><h5>What differs</h5><p>Every system that was asked answered the same.</p></div>";
    }
    if (mitigation) {
      html +=
        "<div><h5>What reproduces it</h5><p>" + inline(mitigation) + "</p></div>";
    }

    // One pane where the shipped default *is* the clause-by-clause
    // reading, which it is since `Dialect::default` moved from
    // `Dialect::Cyclone` to `Dialect::Specification`. Two panes holding
    // the same hex on every row read as two measurements.
    html += '<div class="side">';
    if (DATA["default"] === DATA.specification) {
      html +=
        '<div><span class="caption">how it is, and how it should be — this project, <code>--dialect ' +
        esc(DATA["default"]) +
        "</code>: the shipped default is the clause-by-clause reading of XTypes 1.3</span>" +
        (ours
          ? '<code class="hexline">' + esc(ours) + "</code>"
          : "<p>refused this construct</p>") +
        "</div>";
    } else {
      html +=
        '<div><span class="caption">how it is — this project, <code>--dialect ' +
        esc(DATA["default"]) +
        "</code>, the shipped default</span>" +
        (ours
          ? '<code class="hexline">' + esc(ours) + "</code>"
          : "<p>refused this construct</p>") +
        "</div>";
      html +=
        '<div><span class="caption">how it should be — a clause-by-clause reading of XTypes 1.3, <code>--dialect ' +
        esc(DATA.specification) +
        "</code></span>" +
        (spec
          ? '<code class="hexline">' +
            esc(spec) +
            "</code>" +
            (ours && spec === ours
              ? "<p>Identical to the shipped default here.</p>"
              : "")
          : "<p>refused this construct</p>") +
        "</div>";
    }
    html += "</div>";

    html +=
      '<div class="scroller"><table class="grid"><thead><tr><th>system</th>' +
      "<th>wrote</th><th>octets</th><th>read back</th><th>and it said</th>" +
      "</tr></thead><tbody>";
    for (var s = 0; s < SYS.length; s++) {
      var value = cell(row, s);
      var link = cellLink(row, SYS[s], value);
      var octets =
        value.write === "octets" || value.write === "bytes"
          ? '<code class="hexline">' + esc(value.writeDetail) + "</code>"
          : "";
      var said = "";
      if (value.write === "refused" || VOID[value.write]) {
        said = esc(value.writeDetail);
      }
      if (value.read === "different" && value.readDetail) {
        said +=
          (said ? "<br>" : "") + "read back: " + esc(value.readDetail);
      }
      if (value.warnings > 0) {
        said +=
          (said ? "<br>" : "") +
          '<a href="' +
          esc(path(row, SYS[s], "WARNINGS")) +
          '">' +
          value.warnings +
          " warning(s) from its compiler</a>";
      }
      html +=
        '<tr><th scope="row"><a href="#' +
        SYS[s].id +
        '">' +
        esc(SYS[s].label) +
        "</a></th><td>" +
        '<a href="' +
        esc(link.href) +
        '">' +
        esc(link.label) +
        "</a></td><td>" +
        octets +
        '</td><td><a href="' +
        esc(path(row, SYS[s], "read.tsv")) +
        '">' +
        esc(value.read) +
        "</a></td><td>" +
        said +
        "</td></tr>";
    }
    html += "</tbody></table></div>";
    html += "</div></td></tr>";
    return html;
  }

  function render() {
    var pages = Math.max(1, Math.ceil(filtered.length / PAGE));
    if (page >= pages) page = pages - 1;
    if (page < 0) page = 0;
    var start = page * PAGE;
    var slice = filtered.slice(start, start + PAGE);
    var html = "";
    for (var at = 0; at < slice.length; at++) html += renderRow(slice[at]);
    if (slice.length === 0)
      html =
        '<tr><td colspan="' +
        (SYS.length + 4) +
        '">Nothing matches. The words on a row are the ones to search for: a construct name, a system, a mode, an issue number, or a symptom such as <em>differ</em>, <em>refused</em> or <em>first differing octet</em>.</td></tr>';
    body.innerHTML = html;
    renderCount();
    renderPager(pages);
  }

  function renderPager(pages) {
    var html = "";
    html +=
      '<button type="button" data-page="' +
      (page - 1) +
      '"' +
      (page === 0 ? " disabled" : "") +
      ">← previous</button>";
    var shown = [];
    for (var p = 0; p < pages; p++) {
      if (p < 2 || p >= pages - 2 || Math.abs(p - page) <= 2) shown.push(p);
    }
    var last = -1;
    for (var k = 0; k < shown.length; k++) {
      if (shown[k] > last + 1) html += '<span class="gap">…</span>';
      html +=
        '<button type="button" data-page="' +
        shown[k] +
        '"' +
        (shown[k] === page ? ' aria-current="true"' : "") +
        ">" +
        (shown[k] + 1) +
        "</button>";
      last = shown[k];
    }
    html +=
      '<button type="button" data-page="' +
      (page + 1) +
      '"' +
      (page >= pages - 1 ? " disabled" : "") +
      ">next →</button>";
    pager.innerHTML = html;
  }

  function gotoRow(id) {
    var at = byId[id];
    if (at === undefined) return false;
    // A linked row has to be on screen on arrival, so the filter is
    // cleared rather than the reader being asked to find their way past
    // it. Whatever they were looking at, the link they followed wins.
    if (filtered !== ROWS) {
      query.value = "";
      applyFilter("");
    }
    var position = filtered.indexOf(ROWS[at]);
    if (position === -1) {
      filtered = ROWS;
      position = at;
    }
    page = Math.floor(position / PAGE);
    picked = id;
    open[id] = true;
    render();
    var element = document.getElementById("row-" + id);
    if (element && element.scrollIntoView)
      element.scrollIntoView({ block: "center" });
    return true;
  }

  function setQuery(text) {
    query.value = text;
    applyFilter(text);
    page = 0;
    render();
    var section = document.getElementById("rows");
    if (section && section.scrollIntoView) section.scrollIntoView();
  }

  function applyHash() {
    var hash = location.hash.slice(1);
    if (!hash) return;
    try {
      hash = decodeURIComponent(hash);
    } catch (error) {
      /* a hash that will not decode is used as written */
    }
    var row = /^row-(\d+)$/.exec(hash);
    if (row) {
      gotoRow(parseInt(row[1], 10));
      return;
    }
    if (document.getElementById(hash)) return; // a section: the browser has it
    if (byCase[hash] !== undefined) {
      setQuery(hash);
      return;
    }
    var issue = /^issue-(\d+)$/.exec(hash);
    if (issue) setQuery("#" + issue[1]);
  }

  // ---------------------------------------------------------- events

  var timer = null;
  query.addEventListener("input", function () {
    if (timer) clearTimeout(timer);
    timer = setTimeout(function () {
      applyFilter(query.value);
      page = 0;
      picked = null;
      render();
    }, 120);
  });

  pager.addEventListener("click", function (event) {
    var button = event.target.closest("button[data-page]");
    if (!button || button.disabled) return;
    page = parseInt(button.getAttribute("data-page"), 10);
    render();
    var table = document.getElementById("table");
    if (table && table.scrollIntoView) table.scrollIntoView();
  });

  body.addEventListener("click", function (event) {
    var button = event.target.closest("button.expand");
    if (!button) return;
    var id = parseInt(button.getAttribute("data-id"), 10);
    if (open[id]) delete open[id];
    else open[id] = true;
    render();
  });

  document.addEventListener("click", function (event) {
    var button = event.target.closest("button.filter");
    if (!button) return;
    setQuery(button.getAttribute("data-filter") || "");
  });

  window.addEventListener("hashchange", applyHash);

  // The quick filters are derived rather than written: one per symptom
  // the appendix can be asked about.
  var QUICK = [
    ["differs", "rows where somebody's octets differ from ours"],
    ["refused", "rows somebody would not take"],
    ["misread", "rows somebody read back wrongly"],
    ["unalignable", "rows no flag of ours reproduces"],
    ["warned", "rows a compiler said something about"],
  ];
  var chipHtml = "";
  for (var q = 0; q < QUICK.length; q++) {
    chipHtml +=
      '<button type="button" class="filter" data-filter="' +
      esc(QUICK[q][0]) +
      '" title="' +
      esc(QUICK[q][1]) +
      '">' +
      esc(QUICK[q][0]) +
      "</button>";
  }
  chipHtml +=
    '<button type="button" class="filter" data-filter="" title="every row">clear</button>';
  chips.innerHTML = chipHtml;

  // The theme follows the reader's system unless they say otherwise.
  var toggle = document.createElement("button");
  toggle.id = "theme";
  toggle.type = "button";
  toggle.textContent = "light / dark";
  toggle.addEventListener("click", function () {
    var root = document.documentElement;
    var now = root.getAttribute("data-theme");
    var dark =
      now === "dark" ||
      (!now &&
        window.matchMedia &&
        window.matchMedia("(prefers-color-scheme: dark)").matches);
    root.setAttribute("data-theme", dark ? "light" : "dark");
  });
  document.body.appendChild(toggle);

  render();
  applyHash();
})();
