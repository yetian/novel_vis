<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref, watch } from "vue";
import cytoscape, { type Core, type ElementDefinition } from "cytoscape";
import { useLibraryStore } from "@/stores/library";

const library = useLibraryStore();
const container = ref<HTMLDivElement | null>(null);
const selectedId = ref<number | null>(null);
const query = ref("");
const MAX_NODES = 80;
let cy: Core | null = null;

const timeline = computed(() => library.timeline);
const entities = computed(() => timeline.value?.entities ?? []);
const events = computed(() => timeline.value?.events ?? []);

const eventsByChar = computed(() => {
  const m = new Map<number, number[]>();
  for (const ev of events.value) {
    if (!m.has(ev.entity_id)) m.set(ev.entity_id, []);
    m.get(ev.entity_id)!.push(ev.chapter_index);
  }
  return m;
});

const characters = computed(() => entities.value.filter((e) => e.entity_type === "CHARACTER"));

const keptChars = computed(() => {
  const q = query.value.trim();
  return [...characters.value]
    .filter((c) => (q ? c.name.includes(q) : true))
    .map((c) => ({ c, n: eventsByChar.value.get(c.id)?.length ?? 0 }))
    .sort((a, b) => b.n - a.n)
    .slice(0, MAX_NODES)
    .map((x) => x.c);
});

const charIds = computed(() => new Set(keptChars.value.map((c) => c.id)));

const relationEdges = computed(() => {
  const byChapter = new Map<number, Set<number>>();
  for (const ev of events.value) {
    if (!charIds.value.has(ev.entity_id)) continue;
    if (!byChapter.has(ev.chapter_index)) byChapter.set(ev.chapter_index, new Set());
    byChapter.get(ev.chapter_index)!.add(ev.entity_id);
  }
  const weights = new Map<string, { a: number; b: number; weight: number }>();
  for (const set of byChapter.values()) {
    const ids = [...set].sort((a, b) => a - b);
    for (let i = 0; i < ids.length; i++) {
      for (let j = i + 1; j < ids.length; j++) {
        const key = `${ids[i]}|${ids[j]}`;
        const found = weights.get(key);
        if (found) found.weight += 1;
        else weights.set(key, { a: ids[i], b: ids[j], weight: 1 });
      }
    }
  }
  return [...weights.values()];
});

const nodeId = (id: number) => `c::${id}`;

const neighborIds = computed(() => {
  if (selectedId.value == null) return new Set<number>();
  const set = new Set<number>();
  for (const e of relationEdges.value) {
    if (e.a === selectedId.value) set.add(e.b);
    if (e.b === selectedId.value) set.add(e.a);
  }
  return set;
});

const selectedChar = computed(() => characters.value.find((c) => c.id === selectedId.value) ?? null);

const style = [
  {
    selector: "node",
    style: {
      shape: "ellipse",
      "background-fill": "radial-gradient",
      "background-gradient-stop-colors": "#2bd0a0 #0e5a49",
      "border-color": "#0b0e14",
      "border-width": 2,
      width: "mapData(weight, 0, 30, 34, 84)",
      height: "mapData(weight, 0, 30, 34, 84)",
      label: "data(label)",
      color: "#f2fffb",
      "font-size": 11,
      "font-weight": 600,
      "text-valign": "bottom",
      "text-halign": "center",
      "text-margin-y": 8,
      "text-outline-width": 2,
      "text-outline-color": "#0b0e14",
      "shadow-blur": 16,
      "shadow-color": "#000000",
      "shadow-opacity": 0.45,
      "shadow-offset-y": 3,
    },
  },
  {
    selector: "edge",
    style: {
      "curve-style": "bezier",
      "line-color": "#5b6b86",
      "line-opacity": 0.5,
      width: "mapData(weight, 1, 12, 1, 6)",
      "target-arrow-shape": "none",
    },
  },
  {
    selector: "node.dim",
    style: { opacity: 0.18 },
  },
  {
    selector: "node.active",
    style: { "background-fill": "solid", "background-color": "#f0b429", "border-color": "#fff", color: "#241a00", "z-index": 90 },
  },
  {
    selector: "node.neighbor",
    style: { "border-color": "#f0b429", "border-width": 3, "z-index": 80 },
  },
  {
    selector: "edge.active",
    style: { "line-color": "#f0b429", "line-opacity": 1, "z-index": 70 },
  },
] as unknown as cytoscape.StylesheetJson;

function buildElements(): ElementDefinition[] {
  const els: ElementDefinition[] = [];
  for (const c of keptChars.value) {
    els.push({ data: { id: nodeId(c.id), label: c.name, weight: eventsByChar.value.get(c.id)?.length ?? 0 } });
  }
  for (const e of relationEdges.value) {
    els.push({ data: { id: `e::${e.a}-${e.b}`, source: nodeId(e.a), target: nodeId(e.b), weight: e.weight } });
  }
  return els;
}

function highlight() {
  if (!cy) return;
  cy.nodes().removeClass("active neighbor dim");
  cy.edges().removeClass("active");
  if (selectedId.value == null) return;
  const active = cy.getElementById(nodeId(selectedId.value));
  if (!active.length) return;
  active.addClass("active");
  active.neighborhood("node").addClass("neighbor");
  active.connectedEdges().addClass("active");
  cy.nodes().not(active).not(active.neighborhood("node")).addClass("dim");
}

function rebuild() {
  if (!cy) return;
  cy.elements().remove();
  cy.add(buildElements());
  cy.layout({ name: "cose", animate: false, padding: 60 }).run();
  requestAnimationFrame(() => {
    if (cy && cy.width() > 0) cy.fit(undefined, 60);
  });
  highlight();
}

function init() {
  if (!container.value) return;
  cy = cytoscape({
    container: container.value,
    elements: buildElements(),
    style,
    layout: { name: "cose", animate: false, padding: 60 },
    wheelSensitivity: 0.2,
    minZoom: 0.2,
    maxZoom: 3,
  });
  cy.on("tap", "node", (evt) => {
    selectedId.value = Number(String(evt.target.data("id")).replace("c::", ""));
    highlight();
  });
  cy.on("tap", (evt) => {
    if (evt.target === cy) selectedId.value = null;
    highlight();
  });
  requestAnimationFrame(() => {
    if (cy && cy.width() > 0) cy.fit(undefined, 60);
  });
}

let ro: ResizeObserver | null = null;
onMounted(() => {
  init();
  if (typeof ResizeObserver !== "undefined" && container.value) {
    ro = new ResizeObserver(() => cy?.resize());
    ro.observe(container.value);
  }
  window.addEventListener("resize", onResize);
});
function onResize() {
  cy?.resize();
}
onBeforeUnmount(() => {
  window.removeEventListener("resize", onResize);
  ro?.disconnect();
  cy?.destroy();
  cy = null;
});

watch([keptChars, relationEdges], rebuild);
watch(selectedId, highlight);
</script>

<template>
  <div class="h-full">
    <div v-if="!timeline || !characters.length" class="grid h-full place-items-center p-8 text-center text-slate-500">
      <div>
        <div class="mx-auto mb-4 grid h-16 w-16 place-items-center rounded-2xl bg-ink-700 text-2xl">
          <i class="fa-solid fa-users" />
        </div>
        尚无人物数据。请先抽取该书。
      </div>
    </div>

    <div v-else class="relative h-full">
      <div ref="container" class="h-full w-full" />

      <div class="absolute left-3 top-3 w-64 rounded-lg border border-ink-600/50 bg-ink-800/70 p-3 backdrop-blur">
        <div class="mb-2 text-xs font-medium text-slate-300"><i class="fa-solid fa-user-group mr-1 text-jade-500" />人物关系（同章共现）</div>
        <input v-model="query" class="input !py-1.5 text-xs" placeholder="搜索人物…" />
        <p class="mt-2 text-[11px] text-slate-500">共 {{ characters.length }} 人，展示前 {{ keptChars.length }} 位；连线表示出现在同一章节。</p>
      </div>

      <transition name="fade">
        <div v-if="selectedChar" class="absolute right-3 top-3 w-72 overflow-hidden rounded-xl border border-ink-500/60 bg-ink-800/90 shadow-xl backdrop-blur">
          <div class="flex items-start gap-2 border-b border-ink-600/60 px-4 py-3">
            <i class="fa-solid fa-user mt-0.5 text-jade-400" />
            <div class="min-w-0 flex-1">
              <h4 class="truncate font-semibold text-slate-100">{{ selectedChar.name }}</h4>
              <p class="text-[11px] text-slate-500">
                <template v-if="selectedChar.first_appeared_chapter_index != null">首现 第 {{ selectedChar.first_appeared_chapter_index + 1 }} 章 · </template>
                出场 {{ eventsByChar.get(selectedChar.id)?.length ?? 0 }} 次 · 关联 {{ neighborIds.size }} 人
              </p>
            </div>
            <button class="text-slate-500 hover:text-slate-200" @click="selectedId = null"><i class="fa-solid fa-xmark" /></button>
          </div>
          <div class="max-h-72 overflow-y-auto px-4 py-3 text-xs">
            <p v-if="selectedChar.description" class="mb-2 text-slate-400">{{ selectedChar.description }}</p>
            <div v-if="neighborIds.size" class="mb-2">
              <div class="mb-1 text-[11px] uppercase tracking-wide text-slate-500">关联人物</div>
              <div class="flex flex-wrap gap-1">
                <button
                  v-for="id in [...neighborIds]"
                  :key="id"
                  class="rounded-full border border-jade-500/40 bg-jade-500/10 px-2 py-0.5 text-[11px] text-jade-300 hover:border-jade-400"
                  @click="selectedId = id"
                >
                  {{ characters.find((c) => c.id === id)?.name }}
                </button>
              </div>
            </div>
            <div class="text-[11px] uppercase tracking-wide text-slate-500">出场章节</div>
            <div class="mt-1 flex flex-wrap gap-1">
              <span v-for="ci in [...(eventsByChar.get(selectedChar.id) ?? [])].sort((a, b) => a - b)" :key="ci" class="rounded border border-ink-600 px-1.5 py-0.5 text-slate-400">
                {{ ci + 1 }}
              </span>
            </div>
          </div>
        </div>
      </transition>
    </div>
  </div>
</template>

<style scoped>
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
