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

function buildElements(): ElementDefinition[] {
  const elements: ElementDefinition[] = [];
  const worldSeen = new Set<string>();
  const siteSeen = new Set<string>();

  for (const c of props.chapters) {
    const world = worldOf(c);
    const worldId = `world::${world}`;
    if (!worldSeen.has(worldId)) {
      worldSeen.add(worldId);
      elements.push({ data: { id: worldId, label: world, kind: "world" } });
    }
    const sid = siteId(c);
    if (!siteSeen.has(sid)) {
      siteSeen.add(sid);
      elements.push({ data: { id: sid, label: siteOf(c), kind: "site", parent: worldId } });
    }
  }

  for (let i = 1; i < props.chapters.length; i++) {
    const prev = props.chapters[i - 1];
    const cur = props.chapters[i];
    const src = siteId(prev);
    const dst = siteId(cur);
    if (src !== dst) {
      elements.push({
        data: { id: `edge::${i}`, source: src, target: dst, movement: cur.movement_type || "STAY" },
      });
    }
  }

  elements.push({ data: { id: "token", label: "主角", kind: "token" } });
  return elements;
}

function initGraph() {
  if (!container.value) return;
  cy = cytoscape({
    container: container.value,
    elements: buildElements(),
    style: [
      {
        selector: 'node[kind="world"]',
        style: {
          "background-color": "rgba(240,180,41,0.08)",
          "border-color": "#f0b429",
          "border-width": 1,
          "border-opacity": 0.5,
          label: "data(label)",
          color: "#f0b429",
          "font-size": 12,
          "text-valign": "top",
          "text-margin-y": -8,
          padding: "22px",
          shape: "round-rectangle",
        },
      },
      {
        selector: 'node[kind="site"]',
        style: {
          "background-color": "#20c997",
          label: "data(label)",
          color: "#c4cee0",
          "font-size": 10,
          "text-valign": "bottom",
          "text-margin-y": 4,
          width: 14,
          height: 14,
          "border-color": "#0b0e14",
          "border-width": 1,
        },
      },
      {
        selector: "edge",
        style: {
          "curve-style": "bezier",
          "line-color": "#3a465e",
          "target-arrow-color": "#3a465e",
          "target-arrow-shape": "triangle",
          width: 1.2,
          "arrow-scale": 0.7,
        },
      },
      {
        selector: "node.active",
        style: { "background-color": "#f0b429", width: 24, height: 24, "border-color": "#fff", "border-width": 2, "z-index": 90 },
      },
      {
        selector: "node.trail",
        style: { "background-color": "#f0b429", "border-color": "#f0b429", "border-width": 3 },
      },
      {
        selector: 'node[kind="token"]',
        style: {
          "background-color": "#f03e3e",
          shape: "star",
          width: 26,
          height: 26,
          label: "data(label)",
          color: "#f03e3e",
          "font-size": 10,
          "text-valign": "center",
          "text-margin-y": -18,
          "z-index": 100,
        },
      },
      {
        selector: "edge.active",
        style: { "line-color": "#f0b429", "target-arrow-color": "#f0b429", width: 2.4 },
      },
    ],
    layout: { name: "cose", animate: false, padding: 40 },
  });
  cy.ready(() => cy?.fit(undefined, 60));
  updateActive(false);
}

function updateActive(animate = true) {
  if (!cy) return;
  const chapter = props.chapters[props.activeIndex];
  if (!chapter) return;
  cy.nodes().removeClass("active");
  cy.edges().removeClass("active");
  const node = cy.getElementById(siteId(chapter));
  if (node.length) {
    node.addClass("active");
    const token = cy.getElementById("token");
    if (token.length) token.position(node.position());
    if (animate) {
      cy.animate({ center: { eles: node }, duration: 300 });
    }
  }
  const edge = cy.getElementById(`edge::${props.activeIndex}`);
  if (edge.length) edge.addClass("active");
}

function updateTrail() {
  if (!cy) return;
  cy.nodes().removeClass("trail");
  if (props.highlightEntityId == null) return;
  const chapterIndexes = props.events
    .filter((e) => e.entity_id === props.highlightEntityId)
    .map((e) => e.chapter_index);
  const indexes = chapterIndexes.length ? chapterIndexes : indexOfEntityFirst();
  for (const i of indexes) {
    const chapter = props.chapters.find((c) => c.chapter_index === i);
    if (!chapter) continue;
    const node = cy.getElementById(siteId(chapter));
    if (node.length) node.addClass("trail");
  }
}

function indexOfEntityFirst(): number[] {
  const entity = props.entities.find((e) => e.id === props.highlightEntityId);
  return entity?.first_appeared_chapter_index != null ? [entity.first_appeared_chapter_index] : [];
}

onMounted(initGraph);
onBeforeUnmount(() => {
  cy?.destroy();
  cy = null;
});

watch(() => props.chapters, () => {
  if (cy) {
    cy.elements().remove();
    cy.add(buildElements());
    cy.layout({ name: "cose", animate: false, padding: 40 }).run();
    cy.fit(undefined, 60);
    updateActive(false);
  }
});
watch(() => props.activeIndex, () => updateActive());
watch(() => props.highlightEntityId, updateTrail);
</script>

<template>
  <div ref="container" class="h-full w-full" />
</template>
