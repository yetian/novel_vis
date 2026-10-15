import type { TimelineBundle } from "@/types/schema";

export interface ExportOptions {
  title?: string;
  subtitle?: string;
  cytoscapeCdn?: string;
}

const DEFAULT_CDN = "https://cdn.jsdelivr.net/npm/cytoscape@3.30.2/dist/cytoscape.min.js";

function safeJson(data: unknown): string {
  return JSON.stringify(data).replace(/</g, "\\u003c").replace(/-->/g, "--\\u003e");
}

function escapeHtml(input: string): string {
  return input
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/// 生成零依赖（仅 CDN 引入 Cytoscape.js）的可交互单页 HTML，
/// 内联全部抽取数据，适合直接部署到 GitHub Pages。
export function buildStaticHtml(bundle: TimelineBundle, options: ExportOptions = {}): string {
  const title = options.title ?? `${bundle.book.title} · 时空编年史`;
  const subtitle = options.subtitle ?? (bundle.book.author ? `作者：${bundle.book.author}` : "本地大模型抽取");
  const cdn = options.cytoscapeCdn ?? DEFAULT_CDN;
  const data = safeJson(bundle);

  return `<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>${escapeHtml(title)}</title>
<script src="${cdn}"><\/script>
<style>
  :root { --bg:#0b0e14; --panel:#11151f; --line:#2a3345; --jade:#20c997; --gold:#f0b429; --red:#f03e3e; --text:#dfe6f0; }
  * { box-sizing: border-box; }
  body { margin:0; font-family: "Noto Sans SC","PingFang SC","Microsoft YaHei",system-ui,sans-serif; background:var(--bg); color:var(--text); height:100vh; display:flex; flex-direction:column; }
  header { padding:14px 20px; border-bottom:1px solid var(--line); display:flex; justify-content:space-between; align-items:center; gap:12px; flex-wrap:wrap; }
  header h1 { font-size:18px; margin:0; }
  header .sub { color:#8a97ad; font-size:12px; }
  .badge { font-size:11px; padding:3px 9px; border-radius:999px; background:rgba(32,201,151,.15); color:var(--jade); border:1px solid rgba(32,201,151,.35); }
  main { flex:1; display:grid; grid-template-columns: 1fr 340px; min-height:0; }
  #graph { width:100%; height:100%; background:radial-gradient(600px 300px at 20% 0%, rgba(32,201,151,.08), transparent); }
  aside { border-left:1px solid var(--line); overflow:auto; padding:12px; background:var(--panel); }
  .stage { border-bottom:1px solid var(--line); display:flex; flex-direction:column; background:var(--panel); }
  .stage-head { padding:10px 16px; display:flex; gap:14px; align-items:center; flex-wrap:wrap; }
  input[type=range] { width:100%; accent-color:var(--jade); }
  .chapter-title { font-weight:600; color:var(--gold); }
  .meta { font-size:12px; color:#8a97ad; }
  .pill { display:inline-block; font-size:11px; padding:2px 8px; border-radius:999px; border:1px solid var(--line); margin-right:6px; margin-top:4px; }
  .pill.t { color:var(--jade); border-color:rgba(32,201,151,.4); }
  .pill.s { color:var(--gold); border-color:rgba(240,180,41,.4); }
  .entry { padding:8px 10px; border:1px solid var(--line); border-radius:8px; margin-bottom:8px; cursor:pointer; transition:.15s; }
  .entry:hover { border-color:var(--jade); }
  .entry.active { border-color:var(--jade); background:rgba(32,201,151,.08); }
  .entry .name { font-weight:600; }
  .entry .detail { font-size:11px; color:#8a97ad; margin-top:3px; }
  .sec-title { font-size:12px; text-transform:uppercase; letter-spacing:.08em; color:#8a97ad; margin:14px 4px 8px; }
  .plot { font-size:13px; line-height:1.6; color:#c4cee0; margin:4px 0 0; }
  .chapter-list { max-height:220px; overflow:auto; }
  .chapter-list .row { font-size:12px; padding:5px 8px; border-radius:6px; cursor:pointer; display:flex; gap:8px; }
  .chapter-list .row:hover { background:rgba(255,255,255,.04); }
  .chapter-list .row.active { background:rgba(32,201,151,.12); color:var(--jade); }
  footer { padding:8px 16px; border-top:1px solid var(--line); font-size:11px; color:#66748c; }
  @media (max-width: 900px) { main { grid-template-columns: 1fr; grid-template-rows: 1fr 40vh; } aside { border-left:none; border-top:1px solid var(--line); } }
</style>
</head>
<body>
<header>
  <div>
    <h1>${escapeHtml(title)}</h1>
    <div class="sub">${escapeHtml(subtitle)}</div>
  </div>
  <div class="badge">ChronoSpace Reader · 时空编年史</div>
</header>
<main>
  <div id="graph"></div>
  <aside>
    <div class="sec-title">当前章节</div>
    <div id="chapterPanel"></div>
    <div class="sec-title">人物</div>
    <div id="characters"></div>
    <div class="sec-title">物品 / 功法 / 传承</div>
    <div id="items"></div>
  </aside>
</main>
<div class="stage">
  <div class="stage-head">
    <span class="chapter-title" id="scrubLabel">第 1 章</span>
    <span class="meta" id="scrubMeta"></span>
  </div>
  <div style="padding:0 16px 10px"><input type="range" id="scrubber" min="0" max="0" value="0" /></div>
  <div class="chapter-list" id="chapterList"></div>
</div>
<footer>由 ChronoSpace Reader 导出 · 数据内联，无外部依赖（Cytoscape.js 通过 CDN 加载）</footer>
<script>
const DATA = ${data};
let index = 0;

const chapters = DATA.chapters || [];
const entities = DATA.entities || [];
const events = DATA.events || [];

function entityById(id) { return entities.find(e => e.id === id); }
function firstSite(ch) { return ch.site || (ch.world || "未知空间"); }

// ---------- 时空图谱 ----------
const nodes = new Map();
const elements = [];

function ensureWorld(world) {
  const id = "world::" + world;
  if (!nodes.has(id)) { nodes.set(id, true); elements.push({ data: { id, label: world, kind: "world" } }); }
  return id;
}
function ensureSite(world, site) {
  const id = "site::" + world + "::" + site;
  if (!nodes.has(id)) { nodes.set(id, true); elements.push({ data: { id, label: site, kind: "site", parent: ensureWorld(world) } }); }
  return id;
}

chapters.forEach(ch => {
  const world = ch.world || "未知空间";
  const site = firstSite(ch);
  ensureSite(world, site);
});

chapters.forEach((ch, i) => {
  if (i === 0) return;
  const prev = chapters[i - 1];
  const src = "site::" + (prev.world || "未知空间") + "::" + firstSite(prev);
  const dst = "site::" + (ch.world || "未知空间") + "::" + firstSite(ch);
  if (src !== dst) {
    elements.push({ data: { id: "edge::" + i, source: src, target: dst, movement: ch.movement_type || "STAY", temporal: ch.temporal_type || "" } });
  }
});

const cy = cytoscape({
  container: document.getElementById("graph"),
  elements,
  style: [
    { selector: 'node[kind="world"]', style: { 'background-color': 'rgba(240,180,41,.10)', 'border-color': '#f0b429', 'border-width': 1, label: 'data(label)', color: '#f0b429', 'font-size': 13, 'text-valign': 'top', 'text-margin-y': -6, padding: 18, shape: 'round-rectangle' } },
    { selector: 'node[kind="site"]', style: { 'background-color': '#20c997', label: 'data(label)', color: '#0b0e14', 'font-size': 11, width: 16, height: 16 } },
    { selector: 'edge', style: { 'curve-style': 'bezier', 'line-color': '#3a465e', 'target-arrow-color': '#3a465e', 'target-arrow-shape': 'triangle', width: 1.4, 'arrow-scale': .8 } },
    { selector: 'node.active', style: { 'background-color': '#f0b429', width: 26, height: 26, 'border-color': '#fff', 'border-width': 2, 'z-index': 99 } },
    { selector: 'edge.active', style: { 'line-color': '#f0b429', 'target-arrow-color': '#f0b429', width: 2.5 } },
  ],
  layout: { name: "cose", animate: false, padding: 30 },
});
cy.ready(() => cy.fit(undefined, 40));

// ---------- 章节面板 ----------
const scrubber = document.getElementById("scrubber");
scrubber.max = Math.max(0, chapters.length - 1);

function renderChapter() {
  const ch = chapters[index];
  if (!ch) return;
  document.getElementById("scrubLabel").textContent = "第 " + (ch.chapter_index + 1) + " 章 · " + ch.title;
  document.getElementById("scrubMeta").textContent = (ch.temporal_type || "UNCERTAIN") + " · " + (ch.movement_type || "STAY");
  const activeSiteId = "site::" + (ch.world || "未知空间") + "::" + firstSite(ch);
  cy.nodes().removeClass("active");
  cy.edges().removeClass("active");
  const node = cy.getElementById(activeSiteId);
  if (node.length) { node.addClass("active"); cy.animate({ center: { eles: node }, duration: 300 }); }
  const edge = cy.getElementById("edge::" + index);
  if (edge.length) edge.addClass("active");

  const pills = [];
  if (ch.world) pills.push('<span class="pill">世界: ' + ch.world + '</span>');
  if (ch.region) pills.push('<span class="pill s">区域: ' + ch.region + '</span>');
  if (ch.site) pills.push('<span class="pill s">地点: ' + ch.site + '</span>');
  if (ch.temporal_type) pills.push('<span class="pill t">时间: ' + ch.temporal_type + (ch.time_offset_days ? " (+" + ch.time_offset_days + "天)" : "") + '</span>');
  document.getElementById("chapterPanel").innerHTML =
    '<div style="font-weight:600;margin-bottom:6px">' + ch.title + '</div>' +
    pills.join("") +
    '<p class="plot">' + (ch.raw_text_summary || "（暂无摘要）") + '</p>';

  document.querySelectorAll("#chapterList .row").forEach(r => r.classList.toggle("active", Number(r.dataset.i) === index));
}

function buildChapterList() {
  const list = document.getElementById("chapterList");
  list.innerHTML = chapters.map((ch, i) =>
    '<div class="row" data-i="' + i + '"><span style="color:#66748c;min-width:42px">' + (ch.chapter_index + 1) + '</span><span>' + ch.title + '</span></div>'
  ).join("");
  list.querySelectorAll(".row").forEach(r => r.addEventListener("click", () => { index = Number(r.dataset.i); scrubber.value = String(index); renderChapter(); }));
}

scrubber.addEventListener("input", () => { index = Number(scrubber.value); renderChapter(); });

// ---------- 实体侧栏 ----------
let activeEntity = null;

function renderEntities() {
  const chars = entities.filter(e => e.entity_type === "CHARACTER");
  const items = entities.filter(e => e.entity_type !== "CHARACTER");
  document.getElementById("characters").innerHTML = chars.length
    ? chars.map(e => entryHtml(e, "character")).join("")
    : '<div class="meta">暂无人物数据</div>';
  document.getElementById("items").innerHTML = items.length
    ? items.map(e => entryHtml(e, "item")).join("")
    : '<div class="meta">暂无物品数据</div>';
  document.querySelectorAll(".entry").forEach(el => el.addEventListener("click", () => focusEntity(Number(el.dataset.id))));
}

function entryHtml(e, cls) {
  const first = e.first_appeared_chapter_index != null ? "首次出现：第 " + (e.first_appeared_chapter_index + 1) + " 章" : "";
  return '<div class="entry ' + cls + '" data-id="' + e.id + '"><div class="name">' + e.name + '</div><div class="detail">' + (e.description || first || e.entity_type) + '</div></div>';
}

function focusEntity(id) {
  const entity = entityById(id);
  if (!entity) return;
  activeEntity = id;
  document.querySelectorAll(".entry").forEach(el => el.classList.toggle("active", Number(el.dataset.id) === id));
  const evs = events.filter(ev => ev.entity_id === id);
  const order = evs.map(ev => ev.chapter_index);
  if (order.length) {
    index = Math.min(order[order.length - 1], chapters.length - 1);
  } else if (entity.first_appeared_chapter_index != null) {
    index = entity.first_appeared_chapter_index;
  }
  scrubber.value = String(index);
  renderChapter();
  const originChapter = entity.first_appeared_chapter_index != null ? entity.first_appeared_chapter_index : null;
  if (originChapter != null && chapters[originChapter]) {
    const ch = chapters[originChapter];
    const nodeId = "site::" + (ch.world || "未知空间") + "::" + firstSite(ch);
    const node = cy.getElementById(nodeId);
    if (node.length) { cy.animate({ center: { eles: node }, zoom: 1.4, duration: 400 }); }
  }
  // 高亮该实体触发的章节节点
  cy.edges().removeClass("active");
  evs.forEach(ev => {
    const nd = cy.getElementById("site::" + (chapters[ev.chapter_index] ? (chapters[ev.chapter_index].world || "未知空间") : "未知空间") + "::" + (chapters[ev.chapter_index] ? firstSite(chapters[ev.chapter_index]) : ""));
    if (nd.length) nd.addClass("active");
  });
}

// ---------- 初始化 ----------
buildChapterList();
renderEntities();
renderChapter();
<\/script>
</body>
</html>`;
}
