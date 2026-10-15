<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref, watch } from "vue";
import cytoscape, { type Core, type ElementDefinition } from "cytoscape";
import type { ChapterRow, EntityEventRow, EntityRow, SiteSelection } from "@/types/schema";

const props = defineProps<{
  chapters: ChapterRow[];
  entities: EntityRow[];
  events: EntityEventRow[];
  activeIndex: number;
  highlightEntityId: number | null;
}>();
const emit = defineEmits<{ (e: "select", payload: SiteSelection | null): void }>();

const container = ref<HTMLDivElement | null>(null);
let cy: Core | null = null;

const worldOf = (c: ChapterRow) => (c.world || "未知空间").trim() || "未知空间";
const siteOf = (c: ChapterRow) => (c.site || c.region || worldOf(c)).trim() || worldOf(c);
const worldNodeId = (world: string) => `world::${world}`;
const siteNodeId = (c: ChapterRow) => `site::${worldOf(c)}::${siteOf(c)}`;

const MOVEMENT_COLORS: Record<string, string> = {
  STAY: "#334155",
  LOCAL_MOVE: "#20c997",
  TELEPORT: "#b197fc",
  ASCENSION: "#f0b429",
  DUNGEON_ENTER: "#f03e3e",
  DUNGEON_EXIT: "#4dabf7",
};

// 每个世界一块网格，地点按格子排布，世界横向排列。
const CELL_W = 200;
const CELL_H = 124;
const GAP = 220;

interface Layout {
  positions: Map<string, { x: number; y: number }>;
  weights: Map<string, number>;
}

function computeLayout(): Layout {
  const worldOrder: string[] = [];
  const sitesByWorld = new Map<string, string[]>();
  const weights = new Map<string, number>();
  const seen = new Set<string>();

  for (const c of props.chapters) {
    const world = worldOf(c);
    if (!sitesByWorld.has(world)) {
      sitesByWorld.set(world, []);
      worldOrder.push(world);
    }
    const id = siteNodeId(c);
    if (!seen.has(id)) {
      seen.add(id);
      sitesByWorld.get(world)!.push(id);
      weights.set(id, 0);
    }
    weights.set(id, (weights.get(id) ?? 0) + 1);
  }

  const positions = new Map<string, { x: number; y: number }>();
  let cursorX = 0;
  worldOrder.forEach((world) => {
    const sites = sitesByWorld.get(world)!;
    const n = sites.length;
    const cols = n <= 3 ? Math.max(1, n) : Math.ceil(Math.sqrt(n));
    const rows = Math.ceil(n / cols);
    const blockW = cols * CELL_W;
    const blockH = rows * CELL_H;

    positions.set(worldNodeId(world), { x: cursorX + blockW / 2, y: 0 });
    sites.forEach((id, i) => {
      const col = i % cols;
      const row = Math.floor(i / cols);
      positions.set(id, {
        x: cursorX + col * CELL_W + CELL_W / 2,
        y: -blockH / 2 + row * CELL_H + CELL_H / 2,
      });
    });
    cursorX += blockW + GAP;
  });

  return { positions, weights };
}

function buildElements(): ElementDefinition[] {
  const { positions, weights } = computeLayout();
  const elements: ElementDefinition[] = [];
  const nodeIds = new Set<string>();

  for (const [id, pos] of positions) {
    if (id.startsWith("world::")) {
      elements.push({ data: { id, label: id.slice("world::".length), kind: "world" }, position: pos });
      nodeIds.add(id);
    }
  }
  for (const [id, pos] of positions) {
    if (id.startsWith("site::")) {
      const [, world, site] = id.split("::");
      elements.push({
        data: { id, label: site || "未命名", kind: "site", parent: worldNodeId(world), weight: weights.get(id) ?? 1 },
        position: pos,
      });
      nodeIds.add(id);
    }
  }

  // 仅当两端节点都存在时才创建边，避免 cytoscape 抛错。
  for (let i = 1; i < props.chapters.length; i++) {
    const source = siteNodeId(props.chapters[i - 1]);
    const target = siteNodeId(props.chapters[i]);
    if (source === target) continue;
    if (!nodeIds.has(source) || !nodeIds.has(target)) continue;
    const movement = props.chapters[i].movement_type || "STAY";
    elements.push({
      data: {
        id: `edge::${i}`,
        source,
        target,
        movement,
        color: MOVEMENT_COLORS[movement] || "#334155",
      },
    });
  }

  elements.push({ data: { id: "token", label: "主角", kind: "token" } });
  return elements;
}

const style = [
  {
    selector: 'node[kind="world"]',
    style: {
      "background-color": "rgba(30,41,59,0.55)",
      "border-color": "#64748b",
      "border-width": 1.5,
      shape: "round-rectangle",
      label: "data(label)",
      color: "#fcd34d",
      "font-size": 15,
      "font-weight": 700,
      "text-valign": "top",
      "text-halign": "center",
      "text-margin-y": -12,
      "text-outline-width": 3,
      "text-outline-color": "#0b0e14",
      padding: "32px",
    },
  },
  {
    selector: 'node[kind="site"]',
    style: {
      shape: "round-rectangle",
      "background-fill": "linear-gradient",
      "background-gradient-direction": "to-bottom-right",
      "background-gradient-stop-colors": "#1b6153 #0d332b",
      "border-color": "#3ddc97",
      "border-width": 1.5,
      width: 128,
      height: 46,
      label: "data(label)",
      color: "#f2fffb",
      "font-size": 11,
      "font-weight": 600,
      "text-valign": "center",
      "text-halign": "center",
      "text-wrap": "wrap",
      "text-max-width": "112px",
      "text-outline-width": 2,
      "text-outline-color": "#06231c",
      "shadow-blur": 18,
      "shadow-color": "#000000",
      "shadow-opacity": 0.5,
      "shadow-offset-y": 4,
    },
  },
  {
    selector: "edge",
    style: {
      "curve-style": "bezier",
      "line-color": "data(color)",
      "target-arrow-color": "data(color)",
      "target-arrow-shape": "triangle",
      "arrow-scale": 1,
      width: 2,
      opacity: 0.8,
    },
  },
  {
    selector: 'edge[movement="STAY"]',
    style: { opacity: 0.45, width: 1.5, "line-style": "dashed" },
  },
  {
    selector: "node.active",
    style: {
      "background-fill": "solid",
      "background-color": "#f0b429",
      "border-color": "#ffffff",
      "border-width": 3,
      color: "#241a00",
      "text-outline-width": 0,
      "z-index": 90,
    },
  },
  {
    selector: "node.trail",
    style: { "border-color": "#f0b429", "border-width": 3, "z-index": 80 },
  },
  {
    selector: 'node[kind="token"]',
    style: {
      "background-color": "#ff2d55",
      shape: "star",
      width: 34,
      height: 34,
      label: "data(label)",
      color: "#ffd6dd",
      "font-size": 11,
      "font-weight": 700,
      "text-valign": "center",
      "text-margin-y": -26,
      "text-outline-width": 3,
      "text-outline-color": "#0b0e14",
      "z-index": 100,
    },
  },
  {
    selector: "edge.active",
    style: { "line-color": "#f0b429", "target-arrow-color": "#f0b429", width: 3, opacity: 1, "z-index": 70 },
  },
] as unknown as cytoscape.StylesheetJson;

let ro: ResizeObserver | null = null;
let fittedOnce = false;

function fitNow() {
  if (!cy || cy.width() === 0 || cy.height() === 0) return;
  try {
    cy.resize();
    cy.fit(undefined, 72);
    fittedOnce = true;
  } catch {
    /* ignore */
  }
}

function reconstruct() {
  if (!cy) return;
  try {
    cy.elements().remove();
    cy.add(buildElements());
    cy.layout({ name: "preset", fit: false }).run();
    updateActive(false);
    updateTrail();
    // 容器尺寸可能在挂载后/数据到达后才确定，下一帧再适配。
    requestAnimationFrame(fitNow);
  } catch (e) {
    console.error("图谱渲染失败", e);
  }
}

function initGraph() {
  if (!container.value) return;
  cy = cytoscape({
    container: container.value,
    elements: buildElements(),
    style,
    layout: { name: "preset" },
    wheelSensitivity: 0.2,
    minZoom: 0.2,
    maxZoom: 3,
  });

  cy.on("tap", 'node[kind="site"]', (evt) => {
    const id: string = evt.target.data("id");
    const [, world, site] = id.split("::");
    emit("select", { world, site });
  });
  cy.on("tap", (evt) => {
    if (evt.target === cy) emit("select", null);
  });

  reconstruct();
}

function animateSafely(options: Parameters<Core["animate"]>[0]) {
  if (!cy) return;
  const result = cy.animate(options) as unknown;
  if (result && typeof (result as Promise<unknown>).catch === "function") {
    (result as Promise<unknown>).catch(() => undefined);
  }
}

function updateActive(animate = true) {
  if (!cy) return;
  const chapter = props.chapters[props.activeIndex];
  if (!chapter) return;
  cy.nodes().removeClass("active");
  const node = cy.getElementById(siteNodeId(chapter));
  if (node.length) {
    node.addClass("active");
    const token = cy.getElementById("token");
    if (token.length) token.position(node.position());
    if (animate) animateSafely({ center: { eles: node }, zoom: 1.2, duration: 350 });
  }
  const edge = cy.getElementById(`edge::${props.activeIndex}`);
  if (edge.length) edge.addClass("active");
}

function updateTrail() {
  if (!cy) return;
  cy.nodes().removeClass("trail");
  if (props.highlightEntityId == null) return;
  let indexes = props.events
    .filter((e) => e.entity_id === props.highlightEntityId)
    .map((e) => e.chapter_index);
  if (!indexes.length) {
    const entity = props.entities.find((e) => e.id === props.highlightEntityId);
    if (entity?.first_appeared_chapter_index != null) indexes = [entity.first_appeared_chapter_index];
  }
  for (const i of indexes) {
    const chapter = props.chapters.find((c) => c.chapter_index === i);
    if (!chapter) continue;
    const node = cy.getElementById(siteNodeId(chapter));
    if (node.length) node.addClass("trail");
  }
}

function zoomBy(factor: number) {
  if (!cy) return;
  cy.zoom({ level: cy.zoom() * factor, renderedPosition: { x: cy.width() / 2, y: cy.height() / 2 } });
}
function fit() {
  try {
    cy?.fit(undefined, 72);
  } catch {
    /* ignore */
  }
}

function resize() {
  cy?.resize();
  if (!fittedOnce) fitNow();
}

onMounted(() => {
  initGraph();
  window.addEventListener("resize", resize);
  if (typeof ResizeObserver !== "undefined" && container.value) {
    ro = new ResizeObserver(() => {
      cy?.resize();
      if (!fittedOnce) fitNow();
    });
    ro.observe(container.value);
  }
});
onBeforeUnmount(() => {
  window.removeEventListener("resize", resize);
  ro?.disconnect();
  ro = null;
  cy?.destroy();
  cy = null;
});

watch(() => props.chapters, reconstruct);
watch(() => props.activeIndex, () => updateActive());
watch(() => props.highlightEntityId, updateTrail);
</script>

<template>
  <div class="graph-bg relative h-full w-full">
    <div ref="container" class="h-full w-full" />

    <div class="absolute bottom-3 right-3 flex flex-col gap-1">
      <button class="btn-ghost !p-2" title="放大" @click="zoomBy(1.25)"><i class="fa-solid fa-plus" /></button>
      <button class="btn-ghost !p-2" title="缩小" @click="zoomBy(0.8)"><i class="fa-solid fa-minus" /></button>
      <button class="btn-ghost !p-2" title="适应窗口" @click="fit"><i class="fa-solid fa-expand" /></button>
    </div>

    <div class="pointer-events-none absolute left-3 top-3 rounded-lg border border-ink-600/50 bg-ink-800/60 px-3 py-2 text-[11px] text-slate-400 backdrop-blur">
      <div class="mb-1 font-medium text-slate-300">空间图谱 <span class="text-slate-500">· 点击地点查看详情</span></div>
      <div class="flex flex-wrap gap-x-3 gap-y-1">
        <span><span class="mr-1 inline-block h-2 w-2 rounded-sm" style="background:#3ddc97" />地点</span>
        <span><span class="mr-1 inline-block h-2 w-2 rounded-full" style="background:#f0b429" />当前</span>
        <span><span class="mr-1 inline-block h-2 w-2 rounded-full" style="background:#ff2d55" />主角</span>
        <span><span class="mr-1 inline-block h-0.5 w-3 align-middle" style="background:#b197fc" />传送</span>
        <span><span class="mr-1 inline-block h-0.5 w-3 align-middle" style="background:#f0b429" />飞升</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.graph-bg {
  background-image: radial-gradient(rgba(148, 163, 184, 0.09) 1px, transparent 1px);
  background-size: 22px 22px;
  background-position: -11px -11px;
}
</style>
