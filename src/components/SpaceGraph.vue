<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref, watch } from "vue";
import cytoscape, { type Core, type ElementDefinition } from "cytoscape";
import type { ChapterRow, EntityEventRow, EntityRow } from "@/types/schema";

const props = defineProps<{
  chapters: ChapterRow[];
  entities: EntityRow[];
  events: EntityEventRow[];
  activeIndex: number;
  highlightEntityId: number | null;
}>();

const container = ref<HTMLDivElement | null>(null);
let cy: Core | null = null;

const worldOf = (c: ChapterRow) => c.world || "未知空间";
const siteOf = (c: ChapterRow) => c.site || c.region || worldOf(c);
const siteId = (c: ChapterRow) => `site::${worldOf(c)}::${siteOf(c)}`;

const MOVEMENT_COLORS: Record<string, string> = {
  STAY: "#3a465e",
  LOCAL_MOVE: "#20c997",
  TELEPORT: "#b197fc",
  ASCENSION: "#f0b429",
  DUNGEON_ENTER: "#f03e3e",
  DUNGEON_EXIT: "#4dabf7",
};

interface Layout {
  positions: Map<string, { x: number; y: number }>;
  weights: Map<string, number>;
}

/// 确定性布局：每个世界一行，其下的地点绕世界中心环形排布。
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
    const sid = siteId(c);
    if (!seen.has(sid)) {
      seen.add(sid);
      sitesByWorld.get(world)!.push(sid);
      weights.set(sid, 0);
    }
    weights.set(sid, (weights.get(sid) ?? 0) + 1);
  }

  const maxSites = Math.max(1, ...worldOrder.map((w) => sitesByWorld.get(w)!.length));
  const radius = 90 + maxSites * 12;
  const worldGap = radius * 2 + 180;

  const positions = new Map<string, { x: number; y: number }>();
  worldOrder.forEach((world, wi) => {
    const cx = wi * worldGap;
    const sites = sitesByWorld.get(world)!;
    positions.set(`world::${world}`, { x: cx, y: 0 });
    const n = sites.length;
    sites.forEach((sid, i) => {
      if (n === 1) {
        positions.set(sid, { x: cx, y: 0 });
      } else {
        const angle = -Math.PI / 2 + (i * 2 * Math.PI) / n;
        positions.set(sid, { x: cx + radius * Math.cos(angle), y: radius * Math.sin(angle) });
      }
    });
  });

  return { positions, weights };
}

function buildElements(): ElementDefinition[] {
  const { positions, weights } = computeLayout();
  const elements: ElementDefinition[] = [];

  for (const [id, pos] of positions) {
    if (id.startsWith("world::")) {
      elements.push({ data: { id, label: id.slice(7), kind: "world" }, position: pos });
    }
  }
  for (const [id, pos] of positions) {
    if (id.startsWith("site::")) {
      const label = id.split("::")[2];
      const worldId = `world::${id.split("::")[1]}`;
      elements.push({
        data: { id, label, kind: "site", parent: worldId, weight: weights.get(id) ?? 1 },
        position: pos,
      });
    }
  }

  for (let i = 1; i < props.chapters.length; i++) {
    const prev = props.chapters[i - 1];
    const cur = props.chapters[i];
    const src = siteId(prev);
    const dst = siteId(cur);
    if (src !== dst) {
      elements.push({
        data: {
          id: `edge::${i}`,
          source: src,
          target: dst,
          movement: cur.movement_type || "STAY",
          color: MOVEMENT_COLORS[cur.movement_type || "STAY"] || "#3a465e",
        },
      });
    }
  }

  elements.push({ data: { id: "token", label: "主角", kind: "token" } });
  return elements;
}

const style: cytoscape.StylesheetJson = [
  {
    selector: 'node[kind="world"]',
    style: {
      "background-color": "rgba(240,180,41,0.04)",
      "background-opacity": 1,
      "border-color": "#f0b429",
      "border-opacity": 0.45,
      "border-width": 1,
      "border-style": "dashed",
      shape: "round-rectangle",
      label: "data(label)",
      color: "#f0b429",
      "font-size": 13,
      "font-weight": 600,
      "text-valign": "top",
      "text-halign": "center",
      "text-margin-y": -6,
      padding: "26px",
    },
  },
  {
    selector: 'node[kind="site"]',
    style: {
      "background-color": "#20c997",
      "background-opacity": 0.9,
      "border-color": "#0b0e14",
      "border-width": 2,
      width: "mapData(weight, 1, 12, 18, 46)",
      height: "mapData(weight, 1, 12, 18, 46)",
      label: "data(label)",
      color: "#cdd6e6",
      "font-size": 10,
      "text-valign": "bottom",
      "text-halign": "center",
      "text-margin-y": 6,
      "text-outline-width": 2,
      "text-outline-color": "#0b0e14",
    },
  },
  {
    selector: "edge",
    style: {
      "curve-style": "bezier",
      "line-color": "data(color)",
      "target-arrow-color": "data(color)",
      "target-arrow-shape": "triangle",
      "arrow-scale": 0.9,
      width: 1.6,
      opacity: 0.7,
    },
  },
  {
    selector: "node.active",
    style: {
      "background-color": "#f0b429",
      "border-color": "#ffffff",
      "border-width": 3,
      "z-index": 90,
    },
  },
  {
    selector: "node.trail",
    style: { "border-color": "#f0b429", "border-width": 3, "border-style": "solid", "z-index": 80 },
  },
  {
    selector: 'node[kind="token"]',
    style: {
      "background-color": "#f03e3e",
      shape: "star",
      width: 30,
      height: 30,
      label: "data(label)",
      color: "#ff8787",
      "font-size": 10,
      "font-weight": 700,
      "text-valign": "center",
      "text-margin-y": -22,
      "text-outline-width": 2,
      "text-outline-color": "#0b0e14",
      "z-index": 100,
    },
  },
  {
    selector: "edge.active",
    style: { "line-color": "#f0b429", "target-arrow-color": "#f0b429", width: 3, opacity: 1, "z-index": 70 },
  },
];

function initGraph() {
  if (!container.value) return;
  cy = cytoscape({
    container: container.value,
    elements: buildElements(),
    style,
    layout: { name: "preset", fit: true, padding: 70 },
    wheelSensitivity: 0.2,
    minZoom: 0.2,
    maxZoom: 3,
  });
  updateActive(false);
  updateTrail();
}

function updateActive(animate = true) {
  if (!cy) return;
  const chapter = props.chapters[props.activeIndex];
  if (!chapter) return;
  cy.nodes().removeClass("active");
  const node = cy.getElementById(siteId(chapter));
  if (node.length) {
    node.addClass("active");
    const token = cy.getElementById("token");
    if (token.length) token.position(node.position());
    if (animate) cy.animate({ center: { eles: node }, zoom: 1.15, duration: 350 });
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
    const node = cy.getElementById(siteId(chapter));
    if (node.length) node.addClass("trail");
  }
}

function zoomBy(factor: number) {
  if (!cy) return;
  cy.zoom({ level: cy.zoom() * factor, renderedPosition: { x: cy.width() / 2, y: cy.height() / 2 } });
}
function fit() {
  cy?.fit(undefined, 70);
}

onMounted(() => {
  initGraph();
  window.addEventListener("resize", resize);
});
function resize() {
  cy?.resize();
}
onBeforeUnmount(() => {
  window.removeEventListener("resize", resize);
  cy?.destroy();
  cy = null;
});

watch(
  () => props.chapters,
  () => {
    if (!cy) return;
    cy.elements().remove();
    cy.add(buildElements());
    cy.layout({ name: "preset", fit: true, padding: 70 }).run();
    updateActive(false);
    updateTrail();
  },
);
watch(() => props.activeIndex, () => updateActive());
watch(() => props.highlightEntityId, updateTrail);
</script>

<template>
  <div class="relative h-full w-full">
    <div ref="container" class="h-full w-full" />

    <div class="absolute bottom-3 right-3 flex flex-col gap-1">
      <button class="btn-ghost !p-2" title="放大" @click="zoomBy(1.25)"><i class="fa-solid fa-plus" /></button>
      <button class="btn-ghost !p-2" title="缩小" @click="zoomBy(0.8)"><i class="fa-solid fa-minus" /></button>
      <button class="btn-ghost !p-2" title="适应窗口" @click="fit"><i class="fa-solid fa-expand" /></button>
    </div>

    <div class="pointer-events-none absolute left-3 top-3 rounded-lg border border-ink-600/60 bg-ink-800/70 px-3 py-2 text-[11px] text-slate-400 backdrop-blur">
      <div class="mb-1 font-medium text-slate-300">空间图谱</div>
      <div class="flex flex-wrap gap-x-3 gap-y-1">
        <span><span class="mr-1 inline-block h-2 w-2 rounded-full" style="background:#20c997" />地点</span>
        <span><span class="mr-1 inline-block h-2 w-2 rounded-full" style="background:#f0b429" />世界</span>
        <span><span class="mr-1 inline-block h-2 w-2" style="background:#f03e3e" />主角</span>
      </div>
    </div>
  </div>
</template>
