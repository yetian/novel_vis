<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import type { ChapterRow, EntityEventRow, EntityRow, SiteSelection } from "@/types/schema";

const props = defineProps<{
  chapters: ChapterRow[];
  entities: EntityRow[];
  events: EntityEventRow[];
  activeIndex: number;
  highlightEntityId: number | null;
}>();
const emit = defineEmits<{
  (e: "select", payload: SiteSelection | null): void;
  (e: "select-entity", id: number | null): void;
  (e: "update:activeIndex", value: number): void;
}>();

// 布局：横轴 = 时间（章节），纵轴 = 空间（每行一个地点）。
const ROW_H = 66;
const COL_W = 46;
const LEFT = 176;
const TOP = 44;
const BOTTOM = 28;
const MAX_CHARS = 28;
const MAX_RELATIONS = 180;

// 7 色人物调色板（高对比、深色背景可读）；超过 7 人时循环使用。
const PERSON_COLORS = ["#f0b429", "#3ddc97", "#4dabf7", "#b197fc", "#ff8787", "#ffa94d", "#f783ac"];

const showRelations = ref(true);
const showNames = ref(true);
const scroller = ref<HTMLDivElement | null>(null);
const scrollLeft = ref(0);
const scrollTop = ref(0);

const worldOf = (w?: string | null) => (w || "未知空间").trim() || "未知空间";
const siteOf = (c: { world?: string | null; region?: string | null; site?: string | null }) =>
  (c.site || c.region || worldOf(c.world)).trim() || worldOf(c.world);
const spaceKeyOf = (c: { world?: string | null; region?: string | null; site?: string | null }) =>
  `${worldOf(c.world)}::${(c.region || "").trim()}::${siteOf(c)}`;

interface SpaceRow {
  key: string;
  world: string;
  region: string | null;
  site: string;
  label: string;
  firstIndex: number;
  chapters: number;
}

const spaceRows = computed<SpaceRow[]>(() => {
  const map = new Map<string, SpaceRow>();
  for (const c of props.chapters) {
    const key = spaceKeyOf(c);
    let row = map.get(key);
    if (!row) {
      row = {
        key,
        world: worldOf(c.world),
        region: c.region ?? null,
        site: siteOf(c),
        label: siteOf(c),
        firstIndex: c.chapter_index,
        chapters: 0,
      };
      map.set(key, row);
    }
    row.chapters += 1;
  }
  const rows = [...map.values()].sort((a, b) => a.firstIndex - b.firstIndex);
  // 同名地点补上区域/世界，避免行标签歧义。
  const counts = new Map<string, number>();
  for (const r of rows) counts.set(r.label, (counts.get(r.label) ?? 0) + 1);
  for (const r of rows) {
    if ((counts.get(r.label) ?? 0) > 1) r.label = `${r.site} · ${r.region || r.world}`;
  }
  return rows;
});

const rowIndexByKey = computed(() => {
  const m = new Map<string, number>();
  spaceRows.value.forEach((r, i) => m.set(r.key, i));
  return m;
});

const chapterByIndex = computed(() => {
  const m = new Map<number, ChapterRow>();
  for (const c of props.chapters) m.set(c.chapter_index, c);
  return m;
});

const xOf = (chapterIndex: number) => LEFT + chapterIndex * COL_W + COL_W / 2;
const rowTop = (row: number) => TOP + row * ROW_H;
const rowY = (row: number) => rowTop(row) + ROW_H / 2;

const contentW = computed(() => LEFT + Math.max(1, props.chapters.length) * COL_W + 24);
const contentH = computed(() => TOP + Math.max(1, spaceRows.value.length) * ROW_H + BOTTOM);
const bodyH = computed(() => Math.max(1, spaceRows.value.length) * ROW_H);

const tickStep = computed(() => Math.max(1, Math.ceil(props.chapters.length / 18)));
const ticks = computed(() => props.chapters.filter((_, i) => i % tickStep.value === 0));

const temporalMarks = computed(() =>
  props.chapters
    .filter((c) => c.temporal_type === "JUMP_FORWARD" || c.temporal_type === "FLASHBACK")
    .map((c) => ({
      chapter: c.chapter_index,
      glyph: c.temporal_type === "JUMP_FORWARD" ? "▲" : "▼",
      color: c.temporal_type === "JUMP_FORWARD" ? "#f0b429" : "#4dabf7",
      label: `${c.temporal_type}${c.time_offset_days ? ` · ${c.time_offset_days} 天` : ""}`,
    })),
);

const activeChapter = computed(() => props.chapters[props.activeIndex] ?? null);
const activeX = computed(() => (activeChapter.value ? xOf(activeChapter.value.chapter_index) : 0));

const eventsByChar = computed(() => {
  const m = new Map<number, EntityEventRow[]>();
  for (const ev of props.events) {
    if (!m.has(ev.entity_id)) m.set(ev.entity_id, []);
    m.get(ev.entity_id)!.push(ev);
  }
  for (const arr of m.values()) arr.sort((a, b) => a.chapter_index - b.chapter_index);
  return m;
});

const characters = computed(() => props.entities.filter((e) => e.entity_type === "CHARACTER"));

/// 只追踪出场最多的人物，外加当前高亮人物，避免图形过密。
const keptCharacters = computed(() => {
  const scored = characters.value
    .map((c) => {
      const evs = eventsByChar.value.get(c.id) ?? [];
      const score = evs.length || (c.first_appeared_chapter_index != null ? 1 : 0);
      return { c, score };
    })
    .filter((x) => x.score > 0)
    .sort((a, b) => b.score - a.score);
  const kept = scored.slice(0, MAX_CHARS).map((x) => x.c);
  const highlighted = characters.value.find((c) => c.id === props.highlightEntityId);
  if (highlighted && !kept.some((c) => c.id === highlighted.id)) kept.push(highlighted);
  return kept;
});

interface TrackPoint {
  x: number;
  y: number;
  chapter: number;
  spaceKey: string;
}
interface CharTrack {
  id: number;
  name: string;
  role: string | null;
  color: string;
  points: TrackPoint[];
  turns: TrackPoint[];
  path: string;
  first: TrackPoint | null;
  offset: number;
}

const charTracks = computed<CharTrack[]>(() => {
  return keptCharacters.value.map((c, i) => {
    const evs = eventsByChar.value.get(c.id) ?? [];
    const seen = new Set<number>();
    const raw: { chapter: number; spaceKey: string; role: string | null }[] = [];
    for (const ev of evs) {
      if (seen.has(ev.chapter_index)) continue;
      seen.add(ev.chapter_index);
      const ch = chapterByIndex.value.get(ev.chapter_index);
      if (!ch) continue;
      raw.push({ chapter: ev.chapter_index, spaceKey: spaceKeyOf(ch), role: ev.role ?? null });
    }
    // 首现章节即使没有事件记录，也要标出人物第一次出现的位置。
    if (c.first_appeared_chapter_index != null && !raw.some((p) => p.chapter === c.first_appeared_chapter_index)) {
      const ch = chapterByIndex.value.get(c.first_appeared_chapter_index);
      if (ch) raw.push({ chapter: c.first_appeared_chapter_index, spaceKey: spaceKeyOf(ch), role: null });
    }
    raw.sort((a, b) => a.chapter - b.chapter);

    // 同一空间行内用轻微纵向偏移区分同时出现的不同人物。
    const offset = c.id === props.highlightEntityId ? 0 : ((i * 13) % 21 - 10) * 2;
    const points: TrackPoint[] = [];
    for (const p of raw) {
      const row = rowIndexByKey.value.get(p.spaceKey);
      if (row == null) continue;
      points.push({ x: xOf(p.chapter), y: rowY(row) + offset, chapter: p.chapter, spaceKey: p.spaceKey });
    }
    points.sort((a, b) => a.chapter - b.chapter);

    const path =
      points.length > 1
        ? points.map((p, idx) => `${idx === 0 ? "M" : "L"} ${p.x.toFixed(1)} ${p.y.toFixed(1)}`).join(" ")
        : "";

    const first =
      (c.first_appeared_chapter_index != null
        ? points.find((p) => p.chapter === c.first_appeared_chapter_index)
        : null) ?? points[0] ?? null;

    const turns: TrackPoint[] = [];
    for (let k = 1; k < points.length; k++) {
      if (points[k].spaceKey !== points[k - 1].spaceKey) turns.push(points[k]);
    }
    if (first) turns.unshift(first);

    const role = raw.find((p) => p.role)?.role ?? null;
    const color = PERSON_COLORS[i % PERSON_COLORS.length];
    return { id: c.id, name: c.name, role, color, points, turns, path, first, offset };
  });
});

const pointAt = computed(() => {
  const m = new Map<string, TrackPoint>();
  for (const t of charTracks.value) {
    for (const p of t.points) m.set(`${t.id}|${p.chapter}`, p);
  }
  return m;
});

interface RelationEdge {
  id: string;
  a: number;
  b: number;
  weight: number;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
}

/// 关系 = 同一章节中共同出场；连线画在两人最早共现的时间点上。
const relationEdges = computed<RelationEdge[]>(() => {
  const byChapter = new Map<number, number[]>();
  for (const t of charTracks.value) {
    for (const p of t.points) {
      if (!byChapter.has(p.chapter)) byChapter.set(p.chapter, []);
      byChapter.get(p.chapter)!.push(t.id);
    }
  }

  const weights = new Map<string, { a: number; b: number; weight: number; chapter: number }>();
  for (const [chapter, ids] of byChapter) {
    const unique = [...new Set(ids)].sort((a, b) => a - b);
    for (let i = 0; i < unique.length; i++) {
      for (let j = i + 1; j < unique.length; j++) {
        const key = `${unique[i]}|${unique[j]}`;
        const found = weights.get(key);
        if (found) {
          found.weight += 1;
          if (chapter < found.chapter) found.chapter = chapter;
        } else {
          weights.set(key, { a: unique[i], b: unique[j], weight: 1, chapter });
        }
      }
    }
  }

  let edges: RelationEdge[] = [];
  for (const [key, w] of weights) {
    const pa = pointAt.value.get(`${w.a}|${w.chapter}`);
    const pb = pointAt.value.get(`${w.b}|${w.chapter}`);
    if (!pa || !pb) continue;
    edges.push({ id: `rel::${key}`, a: w.a, b: w.b, weight: w.weight, x1: pa.x, y1: pa.y, x2: pb.x, y2: pb.y });
  }

  if (props.highlightEntityId != null) {
    edges = edges.filter((e) => e.a === props.highlightEntityId || e.b === props.highlightEntityId);
  } else {
    edges.sort((a, b) => b.weight - a.weight);
    edges = edges.slice(0, MAX_RELATIONS);
  }
  return edges;
});

const neighborIds = computed(() => {
  const s = new Set<number>();
  for (const e of relationEdges.value) {
    s.add(e.a);
    s.add(e.b);
  }
  return s;
});

const trackColor = (t: CharTrack) => t.color;
const trackWidth = (t: CharTrack) => {
  if (t.id === props.highlightEntityId) return 3;
  return neighborIds.value.has(t.id) && props.highlightEntityId != null ? 2.2 : 1.6;
};
const trackOpacity = (t: CharTrack) => {
  if (props.highlightEntityId == null) return 0.78;
  if (t.id === props.highlightEntityId) return 1;
  return neighborIds.value.has(t.id) ? 0.66 : 0.1;
};
const dotRadius = (t: CharTrack) => {
  if (t.id === props.highlightEntityId) return 7.5;
  return neighborIds.value.has(t.id) && props.highlightEntityId != null ? 6 : 5;
};
const shortName = (name: string) => (name.length > 8 ? `${name.slice(0, 8)}…` : name);
const relPath = (e: RelationEdge) => {
  const midX = (e.x1 + e.x2) / 2;
  const lift = Math.min(e.y1, e.y2) - 16;
  return `M ${e.x1.toFixed(1)} ${e.y1.toFixed(1)} Q ${midX.toFixed(1)} ${lift.toFixed(1)} ${e.x2.toFixed(1)} ${e.y2.toFixed(1)}`;
};

function selectSpace(row: SpaceRow) {
  emit("select", { world: row.world, region: row.region, site: row.site });
}
function selectTrack(id: number) {
  emit("select-entity", props.highlightEntityId === id ? null : id);
}

function onCanvasClick(e: MouseEvent) {
  const rect = (e.currentTarget as SVGSVGElement).getBoundingClientRect();
  const x = e.clientX - rect.left;
  const y = e.clientY - rect.top;
  if (y < TOP || x < LEFT) return;
  const idx = Math.round((x - LEFT - COL_W / 2) / COL_W);
  const clamped = Math.max(0, Math.min(props.chapters.length - 1, idx));
  emit("update:activeIndex", clamped);
  emit("select-entity", null);
}

function onScroll() {
  const el = scroller.value;
  if (!el) return;
  scrollLeft.value = el.scrollLeft;
  scrollTop.value = el.scrollTop;
}

async function scrollToActive() {
  await nextTick();
  const el = scroller.value;
  if (!el) return;
  const chapter = activeChapter.value;
  if (!chapter) return;
  const x = xOf(chapter.chapter_index);
  const left = el.scrollLeft;
  const right = left + el.clientWidth;
  if (x < left + LEFT + 60) el.scrollTo({ left: Math.max(0, x - LEFT - 60), behavior: "smooth" });
  else if (x > right - 60) el.scrollTo({ left: x - el.clientWidth + 60, behavior: "smooth" });
}

watch(() => props.activeIndex, scrollToActive);
watch(
  () => props.chapters,
  async () => {
    await nextTick();
    scroller.value?.scrollTo({ left: 0, top: 0 });
    scrollLeft.value = 0;
    scrollTop.value = 0;
    requestAnimationFrame(scrollToActive);
  },
);
onMounted(() => requestAnimationFrame(scrollToActive));
</script>

<template>
  <div class="graph-bg relative h-full w-full overflow-hidden">
    <div ref="scroller" class="h-full w-full overflow-auto" @scroll="onScroll">
      <svg
        :width="contentW"
        :height="contentH"
        class="block cursor-crosshair select-none"
        @click="onCanvasClick"
      >
        <!-- 空间行背景 -->
        <g v-for="(row, i) in spaceRows" :key="row.key">
          <rect
            :x="0"
            :y="rowTop(i)"
            :width="contentW"
            :height="ROW_H"
            :fill="i % 2 === 0 ? 'rgba(148,163,184,0.045)' : 'rgba(148,163,184,0.015)'"
          />
          <line
            :x1="0"
            :x2="contentW"
            :y1="rowTop(i) + ROW_H"
            :y2="rowTop(i) + ROW_H"
            stroke="rgba(148,163,184,0.12)"
          />
          <line :x1="LEFT" :x2="LEFT" :y1="rowTop(i)" :y2="rowTop(i) + ROW_H" stroke="rgba(148,163,184,0.18)" />
        </g>

        <!-- 时间网格 -->
        <line
          v-for="c in ticks"
          :key="`grid-${c.id}`"
          :x1="xOf(c.chapter_index)"
          :x2="xOf(c.chapter_index)"
          :y1="TOP"
          :y2="contentH - BOTTOM"
          stroke="rgba(148,163,184,0.07)"
        />

        <!-- 当前章节 -->
        <rect
          v-if="activeChapter"
          :x="activeX - COL_W / 2"
          :y="TOP"
          :width="COL_W"
          :height="bodyH"
          fill="rgba(240,180,41,0.08)"
        />
        <line
          v-if="activeChapter"
          :x1="activeX"
          :x2="activeX"
          :y1="TOP"
          :y2="contentH - BOTTOM"
          stroke="#f0b429"
          stroke-opacity="0.7"
          stroke-width="1.5"
        />

        <!-- 时间跳跃 / 倒叙标记 -->
        <text
          v-for="m in temporalMarks"
          :key="`tm-${m.chapter}`"
          :x="xOf(m.chapter)"
          y="18"
          text-anchor="middle"
          font-size="11"
          font-weight="700"
          :fill="m.color"
        >
          <title>{{ m.label }}</title>
          {{ m.glyph }}
        </text>

        <!-- 人物关系连线 -->
        <g v-if="showRelations">
          <path
            v-for="e in relationEdges"
            :key="e.id"
            :d="relPath(e)"
            fill="none"
            :stroke="highlightEntityId != null ? '#f0b429' : '#94a3b8'"
            :stroke-width="highlightEntityId != null ? 1.6 : 1"
            stroke-dasharray="3 5"
            :opacity="highlightEntityId != null ? 0.75 : 0.32"
          >
            <title>共同出场 {{ e.weight }} 章</title>
          </path>
        </g>

        <!-- 人物轨迹 -->
        <g v-for="t in charTracks" :key="t.id" :opacity="trackOpacity(t)">
          <path
            v-if="t.path"
            :d="t.path"
            fill="none"
            :stroke="trackColor(t)"
            :stroke-width="trackWidth(t)"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
          <path
            v-if="t.path"
            :d="t.path"
            fill="none"
            stroke="transparent"
            stroke-width="14"
            pointer-events="stroke"
            class="cursor-pointer"
            @click.stop="selectTrack(t.id)"
          />
          <circle
            v-for="(p, pi) in t.turns"
            :key="`turn-${pi}`"
            :cx="p.x"
            :cy="p.y"
            r="2.8"
            :fill="trackColor(t)"
            stroke="#0b0e14"
            stroke-width="1"
          />
          <circle
            v-if="t.first"
            :cx="t.first.x"
            :cy="t.first.y"
            :r="dotRadius(t)"
            :fill="trackColor(t)"
            :stroke="t.id === highlightEntityId ? '#ffffff' : '#0b0e14'"
            :stroke-width="t.id === highlightEntityId ? 2.5 : 2"
            class="cursor-pointer"
            @click.stop="selectTrack(t.id)"
          >
            <title>{{ t.name }}{{ t.role ? ` · ${t.role}` : "" }} · 首现第 {{ t.first.chapter + 1 }} 章</title>
          </circle>
          <text
            v-if="showNames && t.first"
            :x="t.first.x + 9"
            :y="t.first.y - 8"
            font-size="10.5"
            font-weight="600"
            :fill="trackColor(t)"
            stroke="#0b0e14"
            stroke-width="3"
            paint-order="stroke"
            style="pointer-events: none"
          >
            {{ shortName(t.name) }}
          </text>
        </g>
      </svg>
    </div>

    <!-- 左侧空间行标签（固定，不随横向滚动） -->
    <div class="pointer-events-none absolute inset-y-0 left-0 overflow-hidden" :style="{ width: `${LEFT}px` }">
      <div
        class="absolute left-0 top-0 w-full"
        :style="{ transform: `translateY(${-scrollTop}px)`, height: `${contentH}px` }"
      >
        <button
          v-for="(row, i) in spaceRows"
          :key="row.key"
          class="pointer-events-auto absolute left-0 flex w-full items-center gap-2 border-b border-ink-600/25 px-3 text-left transition hover:bg-jade-500/10"
          :style="{ top: `${rowTop(i)}px`, height: `${ROW_H}px` }"
          @click.stop="selectSpace(row)"
        >
          <span
            class="flex h-5 w-5 shrink-0 items-center justify-center rounded bg-ink-600/80 text-[10px] font-semibold text-slate-300"
          >
            {{ i + 1 }}
          </span>
          <span class="min-w-0 flex-1">
            <span class="block truncate text-xs font-semibold text-slate-200">{{ row.label }}</span>
            <span class="block truncate text-[10px] text-slate-500">
              {{ row.world }}<template v-if="row.region"> · {{ row.region }}</template> · {{ row.chapters }} 章
            </span>
          </span>
        </button>
      </div>
    </div>

    <!-- 顶部时间轴（固定，不随纵向滚动） -->
    <div class="pointer-events-none absolute left-0 right-0 top-0 overflow-hidden" :style="{ height: `${TOP}px` }">
      <div :style="{ transform: `translateX(${-scrollLeft}px)`, width: `${contentW}px`, height: `${TOP}px` }">
        <div
          v-for="c in ticks"
          :key="`tick-${c.id}`"
          class="absolute top-0 -translate-x-1/2 text-[10px]"
          :class="c.chapter_index === (activeChapter?.chapter_index ?? -1) ? 'text-gold-400' : 'text-slate-500'"
          :style="{ left: `${xOf(c.chapter_index)}px` }"
        >
          {{ c.chapter_index + 1 }}
        </div>
        <div
          class="absolute top-[17px] h-px bg-ink-500/70"
          :style="{ left: `${LEFT}px`, width: `${Math.max(1, chapters.length) * COL_W}px` }"
        />
        <div
          class="absolute top-[14px] h-1.5 w-1.5 rounded-full bg-gold-400"
          :style="{ left: `${activeX}px`, transform: 'translateX(-50%)' }"
        />
      </div>
    </div>

    <!-- 控制与图例 -->
    <div
      class="absolute bottom-3 right-3 rounded-lg border border-ink-600/50 bg-ink-800/85 px-3 py-2 text-[11px] text-slate-400 backdrop-blur"
    >
      <div class="mb-1 flex items-center gap-3">
        <label class="flex cursor-pointer items-center gap-1.5">
          <input v-model="showRelations" type="checkbox" class="accent-jade-500" />
          关系连线
        </label>
        <label class="flex cursor-pointer items-center gap-1.5">
          <input v-model="showNames" type="checkbox" class="accent-jade-500" />
          人名
        </label>
      </div>
      <div class="flex flex-wrap items-center gap-x-3 gap-y-1">
        <span class="inline-flex items-center gap-1">
          <span class="inline-flex gap-0.5">
            <span
              v-for="c in PERSON_COLORS"
              :key="c"
              class="inline-block h-2 w-2 rounded-full"
              :style="{ background: c }"
            />
          </span>
          人物（7 色循环）
        </span>
        <span><span class="mr-1 inline-block h-0.5 w-3 align-middle border-t border-dashed border-slate-400" />关系</span>
      </div>
      <div class="mt-1 text-slate-500">横轴=时间 · 每行=一个空间 · 点击行看地点 · 点击轨迹看人物</div>
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
