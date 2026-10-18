<script setup lang="ts">
import { computed, ref } from "vue";
import EntityDetailDialog from "@/components/EntityDetailDialog.vue";
import { useLibraryStore } from "@/stores/library";
import { biographyKey } from "@/utils/biography";
import { chapterHeading } from "@/utils/chapter";
import {
  computeSpaceImportance,
  computeThingImportance,
  spaceImportanceLabel,
  thingImportanceLabel,
} from "@/utils/people";
import { altWheelToHorizontal } from "@/utils/scroll";
import { cleanPlace, cleanWorld } from "@/utils/space";
import type { ChapterRow, DetailTarget, EntityEventRow, EntityRow } from "@/types/schema";

const library = useLibraryStore();
const tab = ref<"spaces" | "things">("spaces");
const query = ref("");
const mainOnly = ref(false);
const detail = ref<DetailTarget | null>(null);

const timeline = computed(() => library.timeline);
const book = computed(() => timeline.value?.book ?? library.activeBook);
const chapters = computed(() => timeline.value?.chapters ?? []);
const entities = computed(() => timeline.value?.entities ?? []);
const events = computed(() => timeline.value?.events ?? []);

const entityById = computed(() => new Map(entities.value.map((e) => [e.id, e])));
const chapterByIndex = computed(() => {
  const m = new Map<number, ChapterRow>();
  for (const c of chapters.value) m.set(c.chapter_index, c);
  return m;
});

const worldOf = (w?: string | null) => cleanWorld(w);
const siteOf = (c: { world?: string | null; region?: string | null; site?: string | null }) =>
  cleanPlace(c.site) || cleanPlace(c.region) || worldOf(c.world);
const spaceKey = (c: { world?: string | null; region?: string | null; site?: string | null }) =>
  `${worldOf(c.world)}::${cleanPlace(c.region)}::${siteOf(c)}`;

interface SpaceAgg {
  key: string;
  world: string;
  region: string | null;
  site: string;
  chapters: number[];
  chars: Set<number>;
  items: Set<number>;
  charCounts: Map<number, number>;
  itemCounts: Map<number, number>;
  eventCount: number;
  firstChapter: number;
  lastChapter: number;
  explicit: number | null;
  span: number;
  importance: number;
}

const spaces = computed<SpaceAgg[]>(() => {
  const chapterSpace = new Map<number, string>();
  const map = new Map<string, SpaceAgg>();

  for (const c of chapters.value) {
    const site = siteOf(c);
    if (!site) continue;
    const key = spaceKey(c);
    chapterSpace.set(c.chapter_index, key);
    let s = map.get(key);
    if (!s) {
      s = {
        key,
        world: worldOf(c.world),
        region: cleanPlace(c.region) || null,
        site,
        chapters: [],
        chars: new Set(),
        items: new Set(),
        charCounts: new Map(),
        itemCounts: new Map(),
        eventCount: 0,
        firstChapter: c.chapter_index,
        lastChapter: c.chapter_index,
        explicit: null,
        span: 0,
        importance: 1,
      };
      map.set(key, s);
    }
    s.chapters.push(c.chapter_index);
    if (c.spatial_importance != null && c.spatial_importance >= 1 && c.spatial_importance <= 5) {
      const score = Math.round(c.spatial_importance);
      s.explicit = Math.max(s.explicit ?? 0, score);
    }
  }

  for (const ev of events.value) {
    const ent = entityById.value.get(ev.entity_id);
    if (!ent) continue;
    const key = chapterSpace.get(ev.chapter_index);
    if (key) {
      const s = map.get(key);
      if (s) {
        s.eventCount += 1;
        if (ent.entity_type === "CHARACTER") {
          s.chars.add(ev.entity_id);
          s.charCounts.set(ev.entity_id, (s.charCounts.get(ev.entity_id) ?? 0) + 1);
        } else {
          s.items.add(ev.entity_id);
          s.itemCounts.set(ev.entity_id, (s.itemCounts.get(ev.entity_id) ?? 0) + 1);
        }
      }
    }
    if (ev.acquired_site) {
      const acq = cleanPlace(ev.acquired_site);
      if (acq) {
        for (const s of map.values()) {
          if (acq.includes(s.site) || s.site.includes(acq)) {
            s.items.add(ev.entity_id);
            s.itemCounts.set(ev.entity_id, (s.itemCounts.get(ev.entity_id) ?? 0) + 1);
          }
        }
      }
    }
  }

  const base = [...map.values()].map((s) => {
    const indexes = [...s.chapters].sort((a, b) => a - b);
    return {
      ...s,
      chapters: indexes,
      firstChapter: indexes[0] ?? s.firstChapter,
      lastChapter: indexes[indexes.length - 1] ?? s.lastChapter,
    };
  });
  const maxEvents = Math.max(1, ...base.map((s) => s.eventCount));
  const maxChars = Math.max(1, ...base.map((s) => s.chars.size));
  const totalChapters = Math.max(1, chapters.value.length);
  const totalSpan = Math.max(1, chapters.value.length - 1);

  return base
    .map((s) => {
      const span = Math.max(0, s.lastChapter - s.firstChapter);
      const importance = computeSpaceImportance({
        explicit: s.explicit,
        coverage: s.chapters.length / totalChapters,
        volume: s.eventCount / maxEvents,
        spanRatio: span / totalSpan,
        richness: s.chars.size / maxChars,
      });
      return { ...s, span, importance };
    })
    .sort(
      (a, b) =>
        b.importance - a.importance ||
        b.chapters.length - a.chapters.length ||
        b.chars.size - a.chars.size ||
        a.firstChapter - b.firstChapter,
    );
});

const unlocatedChapters = computed(
  () => chapters.value.filter((c) => !siteOf(c)).length,
);

const eventsByEntity = computed(() => {
  const m = new Map<number, EntityEventRow[]>();
  for (const ev of events.value) {
    const list = m.get(ev.entity_id) ?? [];
    list.push(ev);
    m.set(ev.entity_id, list);
  }
  return m;
});

/// 每章出现的具名人物，用于物品的“关联人物”。
const charsByChapter = computed(() => {
  const m = new Map<number, Set<number>>();
  for (const ev of events.value) {
    const ent = entityById.value.get(ev.entity_id);
    if (!ent || ent.entity_type !== "CHARACTER") continue;
    let set = m.get(ev.chapter_index);
    if (!set) {
      set = new Set<number>();
      m.set(ev.chapter_index, set);
    }
    set.add(ev.entity_id);
  }
  return m;
});

interface ThingAgg {
  entity: EntityRow;
  count: number;
  chapterCount: number;
  firstChapter: ChapterRow | null;
  lastChapter: ChapterRow | null;
  acquireChapter: ChapterRow | null;
  acquiredSite: string;
  usage: string;
  related: string[];
  explicit: number | null;
  span: number;
  importance: number;
}

const things = computed<ThingAgg[]>(() => {
  const nonChar = entities.value.filter((e) => e.entity_type !== "CHARACTER");
  const base = nonChar.map((entity) => {
    const evs = eventsByEntity.value.get(entity.id) ?? [];
    const acquire =
      evs.find((e) => e.action_type === "ACQUIRE") ?? evs.find((e) => e.acquired_site) ?? evs[0];
    const chapterSet = new Set(evs.map((e) => e.chapter_index));
    const firstIndex =
      entity.first_appeared_chapter_index ??
      (evs.length ? Math.min(...evs.map((e) => e.chapter_index)) : null);
    const lastIndex = evs.length ? Math.max(...evs.map((e) => e.chapter_index)) : firstIndex;

    const relatedIds = new Set<number>();
    for (const ev of evs) {
      for (const id of charsByChapter.value.get(ev.chapter_index) ?? []) relatedIds.add(id);
    }
    const related = [...relatedIds]
      .map((id) => entityById.value.get(id)?.name ?? "")
      .filter(Boolean)
      .slice(0, 8);
    const explicit =
      entity.importance != null && entity.importance >= 1 && entity.importance <= 5
        ? Math.round(entity.importance)
        : null;

    return {
      entity,
      count: evs.length,
      chapterCount: chapterSet.size,
      firstChapter: firstIndex != null ? chapterByIndex.value.get(firstIndex) ?? null : null,
      lastChapter: lastIndex != null ? chapterByIndex.value.get(lastIndex) ?? null : null,
      acquireChapter:
        acquire != null ? chapterByIndex.value.get(acquire.chapter_index) ?? null : null,
      acquiredSite: cleanPlace(acquire?.acquired_site),
      usage: acquire?.impact_detail || entity.description || "",
      related,
      explicit,
      span:
        firstIndex != null && lastIndex != null ? Math.max(0, lastIndex - firstIndex) : 0,
      importance: 1,
    };
  });
  const maxCount = Math.max(1, ...base.map((t) => t.count));
  const totalChapters = Math.max(1, chapters.value.length);
  const totalSpan = Math.max(1, chapters.value.length - 1);

  return base
    .map((t) => ({
      ...t,
      importance: computeThingImportance({
        explicit: t.explicit,
        coverage: t.chapterCount / totalChapters,
        volume: t.count / maxCount,
        spanRatio: t.span / totalSpan,
      }),
    }))
    .sort(
      (a, b) =>
        b.importance - a.importance ||
        b.count - a.count ||
        (a.firstChapter?.chapter_index ?? Number.MAX_SAFE_INTEGER) -
          (b.firstChapter?.chapter_index ?? Number.MAX_SAFE_INTEGER) ||
        a.entity.name.localeCompare(b.entity.name),
    );
});

const nameOf = (id: number) => entityById.value.get(id)?.name ?? "";
const categoryLabel: Record<string, string> = { ITEM: "物品", SKILL: "功法", LEGACY: "传承" };

const filteredSpaces = computed(() => {
  const base = mainOnly.value
    ? spaces.value.filter((s) => s.importance >= 3)
    : spaces.value;
  const q = query.value.trim();
  if (!q) return base;
  return base.filter(
    (s) =>
      s.site.includes(q) ||
      s.world.includes(q) ||
      (s.region || "").includes(q) ||
      [...s.chars].some((id) => nameOf(id).includes(q)) ||
      [...s.items].some((id) => nameOf(id).includes(q)),
  );
});

const filteredThings = computed(() => {
  const base = mainOnly.value
    ? things.value.filter((t) => t.importance >= 3)
    : things.value;
  const q = query.value.trim();
  if (!q) return base;
  return base.filter(
    (t) =>
      t.entity.name.includes(q) ||
      (t.entity.description || "").includes(q) ||
      t.usage.includes(q) ||
      t.acquiredSite.includes(q) ||
      t.related.some((n) => n.includes(q)),
  );
});

const spaceSubtitle = (s: SpaceAgg) =>
  [s.world, s.region].filter(Boolean).join(" · ");
const firstLocation = (c: ChapterRow | null) =>
  c ? cleanPlace(c.site) || cleanPlace(c.region) || cleanWorld(c.world) : "";
const headingOf = (c?: ChapterRow | null) => (c ? chapterHeading(c, book.value?.title) : "");

const sortedByCount = (ids: Set<number>, counts: Map<number, number>) =>
  [...ids].sort(
    (a, b) => (counts.get(b) ?? 0) - (counts.get(a) ?? 0) || nameOf(a).localeCompare(nameOf(b)),
  );

const hasBiography = (kind: string, key: string) =>
  library.biographyKeys.includes(biographyKey(kind, key));

const placeOf = (chapter: ChapterRow | null) =>
  chapter
    ? [cleanWorld(chapter.world), cleanPlace(chapter.region), cleanPlace(chapter.site)]
        .filter(Boolean)
        .join(" · ")
    : "";

function detailEvent(event: EntityEventRow) {
  const chapter = chapterByIndex.value.get(event.chapter_index) ?? null;
  return {
    id: event.id,
    chapter_index: event.chapter_index,
    chapter_title: chapter ? chapterHeading(chapter, book.value?.title) : "",
    action: event.action_type,
    role: event.role,
    place: placeOf(chapter),
    time_type: chapter?.temporal_type ?? null,
    time_offset_days: chapter?.time_offset_days ?? null,
    detail: event.impact_detail,
  };
}

function openSpaceDetail(space: SpaceAgg) {
  const chapterIndexes = new Set(space.chapters);
  const spaceEvents = events.value
    .filter((event) => chapterIndexes.has(event.chapter_index))
    .map(detailEvent);
  detail.value = {
    kind: "space",
    key: space.key,
    title: space.site,
    subtitle: spaceSubtitle(space),
    icon: "fa-mountain-sun",
    metadata: [
      { label: "世界", value: space.world },
      { label: "区域", value: space.region || "" },
      { label: "重要度", value: `${space.importance}/5 · ${spaceImportanceLabel(space.importance)}` },
      { label: "出现章节", value: `${space.chapters.length} 章 · ${space.eventCount} 次事件` },
      { label: "首次 / 最近", value: `第 ${space.firstChapter + 1} / ${space.lastChapter + 1} 章` },
      { label: "人物", value: [...space.chars].map(nameOf).join("、") },
      { label: "物品 / 功法", value: [...space.items].map(nameOf).join("、") },
    ],
    events: spaceEvents,
  };
}

function openThingDetail(thing: ThingAgg) {
  const thingEvents = (eventsByEntity.value.get(thing.entity.id) ?? []).map(detailEvent);
  detail.value = {
    kind: thing.entity.entity_type.toLowerCase() as DetailTarget["kind"],
    key: thing.entity.name,
    title: thing.entity.name,
    subtitle: thing.entity.description || categoryLabel[thing.entity.entity_type] || "",
    icon: thing.entity.entity_type === "SKILL" ? "fa-hand-fist" : "fa-gem",
    metadata: [
      { label: "类别", value: categoryLabel[thing.entity.entity_type] ?? thing.entity.entity_type },
      { label: "描述", value: thing.entity.description || "" },
      { label: "重要度", value: `${thing.importance}/5 · ${thingImportanceLabel(thing.importance)}` },
      { label: "出现", value: `${thing.count} 次 / ${thing.chapterCount} 章` },
      {
        label: "首次 / 最近",
        value: `${thing.firstChapter ? `第 ${thing.firstChapter.chapter_index + 1} 章` : "—"} / ${
          thing.lastChapter ? `第 ${thing.lastChapter.chapter_index + 1} 章` : "—"
        }`,
      },
      { label: "获得地点", value: thing.acquiredSite },
      { label: "关联人物", value: thing.related.join("、") },
      { label: "使用 / 描述", value: thing.usage },
    ],
    events: thingEvents,
  };
}
</script>

<template>
  <div class="h-full overflow-hidden p-4">
    <div v-if="!timeline || !chapters.length" class="grid h-full place-items-center text-center text-slate-500">
      <div>
        <div class="mx-auto mb-4 grid h-16 w-16 place-items-center rounded-2xl bg-ink-700 text-2xl">
          <i class="fa-solid fa-table-cells-large" />
        </div>
        尚无数据。请先抽取该书。
      </div>
    </div>

    <div v-else class="panel flex h-full min-h-0 flex-col overflow-hidden">
      <header class="flex flex-wrap items-center gap-3 border-b border-ink-600/60 px-4 py-3">
        <div class="min-w-0">
          <h2 class="text-sm font-semibold text-slate-100">
            <i class="fa-solid fa-layer-group mr-1 text-jade-500" />空间与物品总览
          </h2>
          <p class="text-[11px] text-slate-500">
            空间 {{ spaces.length }} · 物品/功法 {{ things.length }} ·
            {{
              tab === "spaces"
                ? "地点按全书重要度排序（模型判断 + 覆盖章节 + 事件频率 + 跨度 + 人物丰富度）"
                : "物品/功法按全书重要度排序（模型判断 + 覆盖章节 + 出现频率 + 跨度）"
            }}
          </p>
          <p v-if="tab === 'spaces' && unlocatedChapters" class="mt-0.5 text-[11px] text-slate-500">
            另有 {{ unlocatedChapters }} 章未识别到空间，未参与重要度排序。
          </p>
        </div>

        <div class="ml-auto flex flex-wrap items-center gap-2">
          <label class="flex cursor-pointer items-center gap-1.5 text-xs text-slate-400">
            <input v-model="mainOnly" type="checkbox" class="accent-jade-500" />
            仅重要
          </label>
          <div class="flex rounded-lg bg-ink-900/60 p-1">
            <button
              class="btn !px-3 !py-1"
              :class="tab === 'spaces' ? 'bg-jade-500 text-ink-900' : 'text-slate-300 hover:bg-ink-600'"
              @click="tab = 'spaces'"
            >
              <i class="fa-solid fa-mountain-sun" />空间 ({{ spaces.length }})
            </button>
            <button
              class="btn !px-3 !py-1"
              :class="tab === 'things' ? 'bg-gold-400 text-ink-900' : 'text-slate-300 hover:bg-ink-600'"
              @click="tab = 'things'"
            >
              <i class="fa-solid fa-gem" />物品功法 ({{ things.length }})
            </button>
          </div>
          <input v-model="query" class="input w-52 !py-1.5 text-xs" placeholder="搜索名称 / 地点 / 人物…" />
          <button v-if="query" class="btn-ghost !px-2 !py-1 text-xs" title="清除搜索" @click="query = ''">
            <i class="fa-solid fa-xmark" />
          </button>
        </div>
      </header>

      <div class="min-h-0 flex-1 overflow-auto" @wheel="altWheelToHorizontal">
        <!-- 空间地点 -->
        <table v-if="tab === 'spaces'" class="w-full table-fixed border-collapse text-left text-sm">
          <thead class="sticky top-0 z-10 bg-ink-800/95 text-[11px] uppercase tracking-wide text-slate-500 backdrop-blur">
            <tr>
              <th class="w-[4%] px-2 py-2 font-medium">#</th>
              <th class="w-[16%] px-2 py-2 font-medium">空间</th>
              <th class="w-[9%] px-2 py-2 font-medium">重要度</th>
              <th class="w-[13%] px-2 py-2 font-medium">首次出现</th>
              <th class="w-[15%] px-2 py-2 font-medium">出现 / 最近</th>
              <th class="w-[21%] px-2 py-2 font-medium">人物</th>
              <th class="w-[22%] px-2 py-2 font-medium">物品 / 功法</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="(s, i) in filteredSpaces"
              :key="s.key"
              class="border-t border-ink-700/50 align-top transition hover:bg-ink-700/20"
            >
              <td class="px-2 py-2.5 text-xs text-slate-600">{{ i + 1 }}</td>

              <td class="px-2 py-2.5">
                <div class="flex items-center gap-2">
                  <button class="flex min-w-0 items-center gap-2 text-left" @click.stop="openSpaceDetail(s)">
                    <i class="fa-solid fa-location-dot text-[11px] text-gold-400" />
                    <span class="truncate font-medium text-slate-100 hover:text-jade-300">{{ s.site }}</span>
                  </button>
                  <button
                    class="text-slate-500 transition hover:text-jade-300"
                    title="查看详情 / 传记"
                    aria-label="查看地点详情"
                    @click.stop="openSpaceDetail(s)"
                  >
                    <i class="fa-solid fa-circle-info text-[11px]" />
                  </button>
                  <i
                    v-if="hasBiography('space', s.key)"
                    class="fa-solid fa-feather-pointed text-[11px] text-gold-400"
                    title="已有传记"
                  />
                </div>
                <div v-if="spaceSubtitle(s)" class="mt-0.5 text-[11px] text-slate-500">{{ spaceSubtitle(s) }}</div>
              </td>

              <td class="px-2 py-2.5">
                <div
                  class="flex items-center gap-0.5"
                  :title="`重要度 ${s.importance}/5${s.explicit != null ? ` · 模型判断 ${s.explicit}/5` : ''} · ${s.chapters.length} 章 / ${s.eventCount} 次事件 / ${s.chars.size} 人`"
                >
                  <span
                    v-for="n in 5"
                    :key="n"
                    class="h-1.5 w-2 rounded-sm"
                    :class="n <= s.importance ? 'bg-gold-400' : 'bg-ink-600'"
                  />
                </div>
                <div class="mt-1 text-[10px] text-slate-500">{{ spaceImportanceLabel(s.importance) }}</div>
              </td>

              <td class="px-2 py-2.5 text-xs">
                <div class="text-slate-300">第 {{ s.firstChapter + 1 }} 章</div>
                <div
                  v-if="headingOf(chapterByIndex.get(s.firstChapter))"
                  class="truncate text-[11px] text-slate-500"
                  :title="headingOf(chapterByIndex.get(s.firstChapter))"
                >
                  {{ headingOf(chapterByIndex.get(s.firstChapter)) }}
                </div>
              </td>

              <td class="px-2 py-2.5 text-xs">
                <div class="text-slate-300">{{ s.chapters.length }} 章 · {{ s.eventCount }} 次事件</div>
                <div class="text-[10px] text-slate-600">
                  首次 {{ s.firstChapter + 1 }} · 最近 {{ s.lastChapter + 1 }}
                </div>
                <div
                  v-if="headingOf(chapterByIndex.get(s.lastChapter))"
                  class="truncate text-[10px] text-slate-500"
                  :title="headingOf(chapterByIndex.get(s.lastChapter))"
                >
                  {{ headingOf(chapterByIndex.get(s.lastChapter)) }}
                </div>
              </td>

              <td class="px-2 py-2.5">
                <div v-if="s.chars.size" class="flex flex-wrap gap-1">
                  <span
                    v-for="id in sortedByCount(s.chars, s.charCounts).slice(0, 8)"
                    :key="id"
                    class="rounded-full border border-jade-500/30 bg-jade-500/5 px-2 py-0.5 text-[10px] text-jade-300"
                    :title="`在本空间出场 ${s.charCounts.get(id) ?? 0} 次`"
                  >
                    {{ nameOf(id) }}<span class="ml-0.5 text-slate-500">{{ s.charCounts.get(id) ?? 0 }}</span>
                  </span>
                  <span v-if="s.chars.size > 8" class="self-center text-[10px] text-slate-500">
                    +{{ s.chars.size - 8 }}
                  </span>
                </div>
                <span v-else class="text-xs text-slate-600">—</span>
              </td>

              <td class="px-2 py-2.5">
                <div v-if="s.items.size" class="flex flex-wrap gap-1">
                  <span
                    v-for="id in sortedByCount(s.items, s.itemCounts).slice(0, 8)"
                    :key="id"
                    class="rounded-full border border-gold-400/30 bg-gold-400/5 px-2 py-0.5 text-[10px] text-gold-400"
                    :title="`在本空间出现 ${s.itemCounts.get(id) ?? 0} 次`"
                  >
                    {{ nameOf(id) }}<span class="ml-0.5 text-slate-500">{{ s.itemCounts.get(id) ?? 0 }}</span>
                  </span>
                  <span v-if="s.items.size > 8" class="self-center text-[10px] text-slate-500">
                    +{{ s.items.size - 8 }}
                  </span>
                </div>
                <span v-else class="text-xs text-slate-600">—</span>
              </td>
            </tr>
          </tbody>
        </table>

        <!-- 物品功法 -->
        <table v-else class="w-full table-fixed border-collapse text-left text-sm">
          <thead class="sticky top-0 z-10 bg-ink-800/95 text-[11px] uppercase tracking-wide text-slate-500 backdrop-blur">
            <tr>
              <th class="w-[4%] px-2 py-2 font-medium">#</th>
              <th class="w-[15%] px-2 py-2 font-medium">名称</th>
              <th class="w-[9%] px-2 py-2 font-medium">重要度</th>
              <th class="w-[7%] px-2 py-2 font-medium">类别</th>
              <th class="w-[12%] px-2 py-2 font-medium">首次出现</th>
              <th class="w-[11%] px-2 py-2 font-medium">获得地点</th>
              <th class="w-[18%] px-2 py-2 font-medium">使用 / 描述</th>
              <th class="w-[16%] px-2 py-2 font-medium">关联人物</th>
              <th class="w-[8%] px-2 py-2 font-medium">出现</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="(t, i) in filteredThings"
              :key="t.entity.id"
              class="border-t border-ink-700/50 align-top transition hover:bg-ink-700/20"
            >
              <td class="px-2 py-2.5 text-xs text-slate-600">{{ i + 1 }}</td>

              <td class="px-2 py-2.5">
                <div class="flex items-center gap-2">
                  <button class="flex min-w-0 items-center gap-2 text-left" @click.stop="openThingDetail(t)">
                    <i
                      class="fa-solid text-[11px] text-gold-400"
                      :class="t.entity.entity_type === 'SKILL' ? 'fa-hand-fist' : 'fa-gem'"
                    />
                    <span class="truncate font-medium text-slate-100 hover:text-jade-300">{{ t.entity.name }}</span>
                  </button>
                  <button
                    class="text-slate-500 transition hover:text-jade-300"
                    title="查看详情 / 传记"
                    aria-label="查看物品详情"
                    @click.stop="openThingDetail(t)"
                  >
                    <i class="fa-solid fa-circle-info text-[11px]" />
                  </button>
                  <i
                    v-if="hasBiography(t.entity.entity_type.toLowerCase(), t.entity.name)"
                    class="fa-solid fa-feather-pointed text-[11px] text-gold-400"
                    title="已有传记"
                  />
                </div>
                <div v-if="t.entity.description" class="mt-0.5 line-clamp-2 text-[11px] text-slate-500">
                  {{ t.entity.description }}
                </div>
              </td>

              <td class="px-2 py-2.5">
                <div
                  class="flex items-center gap-0.5"
                  :title="`重要度 ${t.importance}/5${t.explicit != null ? ` · 模型判断 ${t.explicit}/5` : ''} · 出现 ${t.count} 次 / ${t.chapterCount} 章 / 跨度 ${t.span} 章`"
                >
                  <span
                    v-for="n in 5"
                    :key="n"
                    class="h-1.5 w-2 rounded-sm"
                    :class="n <= t.importance ? 'bg-gold-400' : 'bg-ink-600'"
                  />
                </div>
                <div class="mt-1 text-[10px] text-slate-500">{{ thingImportanceLabel(t.importance) }}</div>
              </td>

              <td class="px-2 py-2.5">
                <span class="rounded border border-gold-400/40 bg-gold-400/10 px-1.5 py-0.5 text-[10px] text-gold-400">
                  {{ categoryLabel[t.entity.entity_type] ?? t.entity.entity_type }}
                </span>
              </td>

              <td class="px-2 py-2.5 text-xs">
                <template v-if="t.firstChapter">
                  <div class="text-slate-300">第 {{ t.firstChapter.chapter_index + 1 }} 章</div>
                  <div v-if="headingOf(t.firstChapter)" class="truncate text-[11px] text-slate-500" :title="headingOf(t.firstChapter)">
                    {{ headingOf(t.firstChapter) }}
                  </div>
                  <div v-if="firstLocation(t.firstChapter)" class="mt-0.5 text-[10px] text-gold-400">
                    <i class="fa-solid fa-location-dot mr-1" />{{ firstLocation(t.firstChapter) }}
                  </div>
                </template>
                <span v-else class="text-slate-600">—</span>
              </td>

              <td class="px-2 py-2.5 text-xs">
                <span v-if="t.acquiredSite" class="text-gold-400">
                  <i class="fa-solid fa-location-dot mr-1" />{{ t.acquiredSite }}
                </span>
                <span v-else class="text-slate-600">—</span>
                <div v-if="t.acquireChapter" class="mt-0.5 text-[10px] text-slate-500">
                  第 {{ t.acquireChapter.chapter_index + 1 }} 章
                  <template v-if="headingOf(t.acquireChapter)"> · {{ headingOf(t.acquireChapter) }}</template>
                </div>
              </td>

              <td class="break-words px-2 py-2.5 text-xs leading-relaxed text-slate-400">
                {{ t.usage || "—" }}
              </td>

              <td class="px-2 py-2.5">
                <div v-if="t.related.length" class="flex flex-wrap gap-1">
                  <span
                    v-for="name in t.related.slice(0, 6)"
                    :key="name"
                    class="rounded-full border border-jade-500/30 bg-jade-500/5 px-2 py-0.5 text-[10px] text-jade-300"
                  >
                    {{ name }}
                  </span>
                  <span v-if="t.related.length > 6" class="self-center text-[10px] text-slate-500">
                    +{{ t.related.length - 6 }}
                  </span>
                </div>
                <span v-else class="text-xs text-slate-600">—</span>
              </td>

              <td class="px-2 py-2.5 text-xs">
                <div class="text-slate-300">{{ t.count }} 次</div>
                <div class="text-[10px] text-slate-600">{{ t.chapterCount }} 章</div>
                <div v-if="t.lastChapter" class="mt-0.5 text-[10px] text-slate-500">
                  最近 {{ t.lastChapter.chapter_index + 1 }} 章
                </div>
              </td>
            </tr>
          </tbody>
        </table>

        <p
          v-if="(tab === 'spaces' ? filteredSpaces.length : filteredThings.length) === 0"
          class="py-10 text-center text-sm text-slate-500"
        >
          {{ tab === "spaces" ? "无匹配地点。" : "无匹配物品。" }}
        </p>
      </div>
    </div>

    <EntityDetailDialog
      :open="!!detail"
      :target="detail"
      :book-id="book?.id ?? null"
      @close="detail = null"
    />
  </div>
</template>
