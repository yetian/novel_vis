<script setup lang="ts">
import { computed, ref } from "vue";
import EntityDetailDialog from "@/components/EntityDetailDialog.vue";
import { useLibraryStore } from "@/stores/library";
import { biographyKey } from "@/utils/biography";
import { chapterHeading } from "@/utils/chapter";
import { computeImportance, importanceLabel } from "@/utils/people";
import { altWheelToHorizontal } from "@/utils/scroll";
import { cleanPlace, cleanWorld } from "@/utils/space";
import type { ChapterRow, DetailRole, DetailTarget, EntityEventRow, EntityRow } from "@/types/schema";

const library = useLibraryStore();
const query = ref("");
const mainOnly = ref(false);
const detail = ref<DetailTarget | null>(null);

const timeline = computed(() => library.timeline);
const book = computed(() => timeline.value?.book ?? library.activeBook);
const chapters = computed(() => timeline.value?.chapters ?? []);
const entities = computed(() => timeline.value?.entities ?? []);
const events = computed(() => timeline.value?.events ?? []);

const normalizeName = (s?: string | null) => (s || "").replace(/[\s·•,，.。:：()（）\-—_]/g, "").toLowerCase();
const bookAuthor = computed(() => (book.value?.author || "").trim());
const authorKey = computed(() => normalizeName(bookAuthor.value));
const AUTHOR_LABELS = new Set(["作者", "本书作者", "原作者"]);

/// 人物：排除作者署名与“作者”占位条目。
const characters = computed(() =>
  entities.value.filter((e) => {
    if (e.entity_type !== "CHARACTER") return false;
    const name = (e.name || "").trim();
    if (AUTHOR_LABELS.has(name)) return false;
    if (authorKey.value && normalizeName(name) === authorKey.value) return false;
    return true;
  }),
);

const chapterByIndex = computed(() => {
  const m = new Map<number, ChapterRow>();
  for (const c of chapters.value) m.set(c.chapter_index, c);
  return m;
});

const eventsByChar = computed(() => {
  const m = new Map<number, EntityEventRow[]>();
  for (const ev of events.value) {
    const list = m.get(ev.entity_id) ?? [];
    list.push(ev);
    m.set(ev.entity_id, list);
  }
  return m;
});

const characterIds = computed(() => new Set(characters.value.map((c) => c.id)));

/// 人物最具代表性的身份/关系：取事件中出现最多的非空 role。
const roleByChar = computed(() => {
  const counts = new Map<number, Map<string, number>>();
  for (const ev of events.value) {
    if (!ev.role) continue;
    let m = counts.get(ev.entity_id);
    if (!m) {
      m = new Map<string, number>();
      counts.set(ev.entity_id, m);
    }
    m.set(ev.role, (m.get(ev.role) ?? 0) + 1);
  }
  const result = new Map<number, string>();
  for (const [id, m] of counts) {
    const best = [...m.entries()].sort((a, b) => b[1] - a[1])[0];
    if (best) result.set(id, best[0]);
  }
  return result;
});

/// 每个角色第一次出现、最近出现、地点与完整轨迹。
const roleByChapterChar = computed(() => {
  const m = new Map<string, Map<string, number>>();
  for (const ev of events.value) {
    if (!ev.role) continue;
    const key = `${ev.chapter_index}|${ev.entity_id}`;
    let counts = m.get(key);
    if (!counts) {
      counts = new Map();
      m.set(key, counts);
    }
    counts.set(ev.role, (counts.get(ev.role) ?? 0) + 1);
  }
  return m;
});

const LEVEL_UP_RE = /晋升|升为|成为|接任|晋级|提升|升级|promoted|appointed/i;

const rolesByChar = computed(() => {
  const result = new Map<number, DetailRole[]>();
  const byChar = new Map<number, Map<string, EntityEventRow[]>>();
  for (const ev of events.value) {
    if (!ev.role || !characterIds.value.has(ev.entity_id)) continue;
    let roles = byChar.get(ev.entity_id);
    if (!roles) {
      roles = new Map();
      byChar.set(ev.entity_id, roles);
    }
    const list = roles.get(ev.role) ?? [];
    list.push(ev);
    roles.set(ev.role, list);
  }
  for (const [entityId, roleMap] of byChar) {
    const roles: DetailRole[] = [...roleMap.entries()]
      .map(([name, evs]) => {
        evs.sort((a, b) => a.chapter_index - b.chapter_index || a.id - b.id);
        const first = evs[0];
        const last = evs[evs.length - 1];
        const firstChapter = chapterByIndex.value.get(first.chapter_index) ?? null;
        const levelUp = evs.some((ev) =>
          LEVEL_UP_RE.test(`${ev.role ?? ""} ${ev.impact_detail ?? ""}`),
        );
        return {
          name,
          first_chapter: first.chapter_index,
          first_chapter_title: firstChapter ? chapterHeading(firstChapter, book.value?.title) : "",
          first_place: firstChapter
            ? [cleanWorld(firstChapter.world), cleanPlace(firstChapter.region), cleanPlace(firstChapter.site)]
                .filter(Boolean)
                .join(" · ")
            : "",
          last_chapter: last.chapter_index,
          count: evs.length,
          level_up: levelUp,
          trace: evs.map((ev) => {
            const chapter = chapterByIndex.value.get(ev.chapter_index) ?? null;
            return {
              chapter_index: ev.chapter_index,
              chapter_title: chapter ? chapterHeading(chapter, book.value?.title) : "",
              place: chapter
                ? [cleanWorld(chapter.world), cleanPlace(chapter.region), cleanPlace(chapter.site)]
                    .filter(Boolean)
                    .join(" · ")
                : "",
              action: ev.action_type,
              detail: ev.impact_detail,
            };
          }),
        };
      })
      .sort((a, b) => a.first_chapter - b.first_chapter || b.count - a.count);
    // 身份顺序变化时标记为升级/变化，便于画出身份演进线。
    roles.forEach((role, index) => {
      if (index > 0) role.level_up = true;
    });
    result.set(entityId, roles);
  }
  return result;
});

/// 关系 = 同一章节共同出场；权重为共现章节数，并记录双方在该章的身份。
const relationsByChar = computed(() => {
  const byChapter = new Map<number, Set<number>>();
  for (const ev of events.value) {
    if (!characterIds.value.has(ev.entity_id)) continue;
    let set = byChapter.get(ev.chapter_index);
    if (!set) {
      set = new Set<number>();
      byChapter.set(ev.chapter_index, set);
    }
    set.add(ev.entity_id);
  }

  const topRole = (counts?: Map<string, number>) => {
    const best = [...(counts?.entries() ?? [])].sort((a, b) => b[1] - a[1])[0];
    return best?.[0] ?? null;
  };
  const bump = (counts: Map<string, number>, role: string | null | undefined) => {
    if (role) counts.set(role, (counts.get(role) ?? 0) + 1);
  };

  const weights = new Map<
    string,
    {
      a: number;
      b: number;
      weight: number;
      first: number;
      last: number;
      aRoles: Map<string, number>;
      bRoles: Map<string, number>;
    }
  >();
  for (const [chapter, ids] of byChapter) {
    const list = [...ids].sort((a, b) => a - b);
    for (let i = 0; i < list.length; i++) {
      for (let j = i + 1; j < list.length; j++) {
        const key = `${list[i]}|${list[j]}`;
        const aRole = topRole(roleByChapterChar.value.get(`${chapter}|${list[i]}`));
        const bRole = topRole(roleByChapterChar.value.get(`${chapter}|${list[j]}`));
        const found = weights.get(key);
        if (found) {
          found.weight += 1;
          found.first = Math.min(found.first, chapter);
          found.last = Math.max(found.last, chapter);
          bump(found.aRoles, aRole);
          bump(found.bRoles, bRole);
        } else {
          weights.set(key, {
            a: list[i],
            b: list[j],
            weight: 1,
            first: chapter,
            last: chapter,
            aRoles: new Map(),
            bRoles: new Map(),
          });
          const created = weights.get(key)!;
          bump(created.aRoles, aRole);
          bump(created.bRoles, bRole);
        }
      }
    }
  }

  const nameById = new Map(characters.value.map((c) => [c.id, c.name]));
  const result = new Map<
    number,
    {
      id: number;
      name: string;
      weight: number;
      first: number;
      last: number;
      selfRole: string | null;
      otherRole: string | null;
    }[]
  >();
  const ensure = (id: number) => {
    let list = result.get(id);
    if (!list) {
      list = [];
      result.set(id, list);
    }
    return list;
  };
  for (const w of weights.values()) {
    const an = nameById.get(w.a);
    const bn = nameById.get(w.b);
    if (!an || !bn) continue;
    ensure(w.a).push({
      id: w.b,
      name: bn,
      weight: w.weight,
      first: w.first,
      last: w.last,
      selfRole: topRole(w.aRoles),
      otherRole: topRole(w.bRoles),
    });
    ensure(w.b).push({
      id: w.a,
      name: an,
      weight: w.weight,
      first: w.first,
      last: w.last,
      selfRole: topRole(w.bRoles),
      otherRole: topRole(w.aRoles),
    });
  }
  for (const list of result.values()) list.sort((a, b) => b.weight - a.weight || a.name.localeCompare(b.name));
  return result;
});

/// 章节累计虚拟天数，用于展示人物首次出现时的“故事时间”。
const dayByChapter = computed(() => {
  let acc = 0;
  const m = new Map<number, number>();
  for (const c of chapters.value) {
    acc += c.time_offset_days ?? 0;
    m.set(c.chapter_index, acc);
  }
  return m;
});
const hasStoryTime = computed(() => chapters.value.some((c) => (c.time_offset_days ?? 0) !== 0));

interface PersonRow {
  entity: EntityRow;
  role: string;
  importance: number;
  explicit: number | null;
  eventCount: number;
  chapterCount: number;
  span: number;
  firstChapter: ChapterRow | null;
  lastChapter: ChapterRow | null;
  firstDay: number | null;
  firstPlace: string;
  firstWhere: string;
  topPlaces: { name: string; chapters: number }[];
  relations: {
    id: number;
    name: string;
    weight: number;
    first: number;
    last: number;
    selfRole: string | null;
    otherRole: string | null;
  }[];
}

/// 先汇总全书范围，再用“模型判断 + 覆盖章节 + 出场频率 + 时间跨度”计算全局重要度。
const globalStats = computed(() => {
  const list = characters.value.map((entity) => {
    const evs = eventsByChar.value.get(entity.id) ?? [];
    const chapterSet = new Set(evs.map((e) => e.chapter_index));
    let first = entity.first_appeared_chapter_index ?? null;
    let last = first;
    for (const ev of evs) {
      if (first == null || ev.chapter_index < first) first = ev.chapter_index;
      if (last == null || ev.chapter_index > last) last = ev.chapter_index;
    }
    return { entity, evs, chapterSet, first, last };
  });
  return {
    list,
    maxEvents: list.reduce((max, x) => Math.max(max, x.evs.length), 1),
    totalChapters: Math.max(1, chapters.value.length),
    totalSpan: Math.max(1, chapters.value.length - 1),
  };
});

const allRows = computed<PersonRow[]>(() => {
  const stats = globalStats.value;
  return stats.list
    .map(({ entity, evs, chapterSet, first, last }) => {
      const eventCount = evs.length;
      const chapterCount = chapterSet.size;
      const span = first != null && last != null ? Math.max(0, last - first) : 0;
      const firstIndex = entity.first_appeared_chapter_index ?? first;
      const firstChapter = firstIndex != null ? chapterByIndex.value.get(firstIndex) ?? null : null;
      const role = roleByChar.value.get(entity.id) ?? "";
      const explicit =
        entity.importance != null && entity.importance >= 1 && entity.importance <= 5
          ? Math.round(entity.importance)
          : null;

      const coverage = chapterCount / stats.totalChapters;
      const volume = eventCount / stats.maxEvents;
      const spanRatio = span / stats.totalSpan;
      const importance = computeImportance({ explicit, coverage, volume, spanRatio, role });

      const firstPlace = firstChapter
        ? cleanPlace(firstChapter.site) ||
          cleanPlace(firstChapter.region) ||
          cleanWorld(firstChapter.world) ||
          ""
        : "";
      const firstWhere = firstChapter
        ? [cleanWorld(firstChapter.world), cleanPlace(firstChapter.region)].filter(Boolean).join(" · ")
        : "";

      const lastIndex = last ?? first;
      const lastChapter =
        lastIndex != null ? chapterByIndex.value.get(lastIndex) ?? null : null;

      const placeChapters = new Map<string, Set<number>>();
      for (const ev of evs) {
        const ch = chapterByIndex.value.get(ev.chapter_index);
        if (!ch) continue;
        const name =
          cleanPlace(ch.site) ||
          cleanPlace(ch.region) ||
          cleanWorld(ch.world) ||
          "未标注空间";
        let set = placeChapters.get(name);
        if (!set) {
          set = new Set<number>();
          placeChapters.set(name, set);
        }
        set.add(ev.chapter_index);
      }
      const topPlaces = [...placeChapters.entries()]
        .map(([name, set]) => ({ name, chapters: set.size }))
        .sort((a, b) => b.chapters - a.chapters || a.name.localeCompare(b.name))
        .slice(0, 2);

      return {
        entity,
        role,
        importance,
        explicit,
        eventCount,
        chapterCount,
        span,
        firstChapter,
        lastChapter,
        firstDay:
          firstIndex != null ? dayByChapter.value.get(firstIndex) ?? null : null,
        firstPlace,
        firstWhere,
        topPlaces,
        relations: relationsByChar.value.get(entity.id) ?? [],
      };
    })
    .sort((a, b) => {
      if (b.importance !== a.importance) return b.importance - a.importance;
      if (b.chapterCount !== a.chapterCount) return b.chapterCount - a.chapterCount;
      if (b.eventCount !== a.eventCount) return b.eventCount - a.eventCount;
      if (b.span !== a.span) return b.span - a.span;
      const ai = a.firstChapter?.chapter_index ?? Number.MAX_SAFE_INTEGER;
      const bi = b.firstChapter?.chapter_index ?? Number.MAX_SAFE_INTEGER;
      if (ai !== bi) return ai - bi;
      return a.entity.name.localeCompare(b.entity.name);
    });
});

const relationTotal = computed(() =>
  Math.round(allRows.value.reduce((sum, r) => sum + r.relations.length, 0) / 2),
);

const rows = computed(() => {
  const q = query.value.trim();
  const base = mainOnly.value ? allRows.value.filter((r) => r.importance >= 3) : allRows.value;
  if (!q) return base;
  return base.filter(
    (r) =>
      r.entity.name.includes(q) ||
      (r.entity.description || "").includes(q) ||
      r.role.includes(q) ||
      r.topPlaces.some((p) => p.name.includes(q)) ||
      r.relations.some((rel) => rel.name.includes(q)),
  );
});

const hasBiography = (row: PersonRow) =>
  library.biographyKeys.includes(biographyKey("character", row.entity.name));

const placeOf = (chapter: ChapterRow | null) =>
  chapter
    ? [cleanWorld(chapter.world), cleanPlace(chapter.region), cleanPlace(chapter.site)]
        .filter(Boolean)
        .join(" · ")
    : "";

function openDetail(row: PersonRow) {
  const events = (eventsByChar.value.get(row.entity.id) ?? []).map((event) => {
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
  });
  const roles = rolesByChar.value.get(row.entity.id) ?? [];
  const roleNames = roles.map((role) => role.name);
  detail.value = {
    kind: "character",
    key: row.entity.name,
    title: row.entity.name,
    subtitle: row.role || row.entity.description || "",
    icon: "fa-user",
    metadata: [
      { label: "简介", value: row.entity.description || "" },
      { label: "身份 / 关系", value: roleNames.join("、") },
      { label: "重要度", value: `${row.importance}/5 · ${importanceLabel(row.importance)}` },
      {
        label: "首次出场",
        value: row.firstChapter
          ? `第 ${row.firstChapter.chapter_index + 1} 章 · ${chapterHeading(row.firstChapter, book.value?.title)}`
          : "",
      },
      {
        label: "最近出场",
        value: row.lastChapter
          ? `第 ${row.lastChapter.chapter_index + 1} 章 · ${chapterHeading(row.lastChapter, book.value?.title)}`
          : "",
      },
      { label: "出场跨度", value: `${row.eventCount} 次事件 / ${row.chapterCount} 章 / 跨度 ${row.span} 章` },
      { label: "主要活动地", value: row.topPlaces.map((p) => p.name).join("、") },
      { label: "关系", value: row.relations.map((rel) => rel.name).join("、") },
    ],
    roles,
    relations: row.relations.map((rel) => ({
      name: rel.name,
      self_role: rel.selfRole,
      other_role: rel.otherRole,
      weight: rel.weight,
      first_chapter: rel.first,
      last_chapter: rel.last,
    })),
    events,
  };
}
</script>

<template>
  <div class="h-full overflow-hidden p-4">
    <div v-if="!timeline || !characters.length" class="grid h-full place-items-center p-8 text-center text-slate-500">
      <div>
        <div class="mx-auto mb-4 grid h-16 w-16 place-items-center rounded-2xl bg-ink-700 text-2xl">
          <i class="fa-solid fa-users" />
        </div>
        尚无人物数据。请先抽取该书。
      </div>
    </div>

    <div v-else class="panel flex h-full min-h-0 flex-col overflow-hidden">
      <header class="flex flex-wrap items-center gap-3 border-b border-ink-600/60 px-4 py-3">
        <div class="min-w-0">
          <h2 class="text-sm font-semibold text-slate-100">
            <i class="fa-solid fa-users mr-1 text-jade-500" />人物总览
          </h2>
          <p class="text-[11px] text-slate-500">
            共 {{ characters.length }} 人 · {{ relationTotal }} 段同章关系 · 重要度按全书汇总（模型判断 + 覆盖章节 + 出场频率 + 时间跨度）· 已排除作者署名
          </p>
        </div>
        <div class="ml-auto flex items-center gap-2">
          <label class="flex cursor-pointer items-center gap-1.5 text-xs text-slate-400">
            <input v-model="mainOnly" type="checkbox" class="accent-jade-500" />
            仅主要人物
          </label>
          <input v-model="query" class="input w-52 !py-1.5 text-xs" placeholder="搜索姓名 / 简介 / 地点 / 关系…" />
          <button v-if="query" class="btn-ghost !px-2 !py-1 text-xs" title="清除搜索" @click="query = ''">
            <i class="fa-solid fa-xmark" />
          </button>
        </div>
      </header>

      <div class="min-h-0 flex-1 overflow-auto" @wheel="altWheelToHorizontal">
        <table class="w-full table-fixed border-collapse text-left text-sm">
          <thead class="sticky top-0 z-10 bg-ink-800/95 text-[11px] uppercase tracking-wide text-slate-500 backdrop-blur">
            <tr>
              <th class="w-[4%] px-2 py-2 font-medium">#</th>
              <th class="w-[12%] px-2 py-2 font-medium">人物</th>
              <th class="w-[9%] px-2 py-2 font-medium">重要度</th>
              <th class="w-[18%] px-2 py-2 font-medium">简介</th>
              <th class="w-[14%] px-2 py-2 font-medium">出场跨度</th>
              <th class="w-[14%] px-2 py-2 font-medium">首现时空</th>
              <th class="w-[12%] px-2 py-2 font-medium">主要活动地</th>
              <th class="w-[17%] px-2 py-2 font-medium">关系</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="(row, i) in rows"
              :key="row.entity.id"
              class="border-t border-ink-700/50 align-top transition hover:bg-ink-700/20"
            >
              <td class="px-2 py-2.5 text-xs text-slate-600">{{ i + 1 }}</td>

              <td class="px-2 py-2.5">
                <div class="flex items-center gap-2">
                  <button class="flex min-w-0 items-center gap-2 text-left" @click.stop="openDetail(row)">
                    <i class="fa-solid fa-user text-[11px] text-jade-400" />
                    <span class="truncate font-medium text-slate-100 hover:text-jade-300">{{ row.entity.name }}</span>
                  </button>
                  <button
                    class="text-slate-500 transition hover:text-jade-300"
                    title="查看详情 / 传记"
                    aria-label="查看详情"
                    @click.stop="openDetail(row)"
                  >
                    <i class="fa-solid fa-circle-info text-[11px]" />
                  </button>
                  <i
                    v-if="hasBiography(row)"
                    class="fa-solid fa-feather-pointed text-[11px] text-gold-400"
                    title="已有传记"
                  />
                </div>
                <div v-if="row.role" class="mt-0.5 text-[11px] text-jade-400">{{ row.role }}</div>
              </td>

              <td class="px-2 py-2.5">
                <div
                  class="flex items-center gap-0.5"
                  :title="`重要度 ${row.importance}/5${row.explicit != null ? ` · 模型判断 ${row.explicit}/5` : ''} · 出场 ${row.eventCount} 次 / ${row.chapterCount} 章`"
                >
                  <span
                    v-for="n in 5"
                    :key="n"
                    class="h-1.5 w-2 rounded-sm"
                    :class="n <= row.importance ? 'bg-gold-400' : 'bg-ink-600'"
                  />
                </div>
                <div class="mt-1 text-[10px] text-slate-500">{{ importanceLabel(row.importance) }}</div>
              </td>

              <td class="break-words px-2 py-2.5 text-xs leading-relaxed text-slate-400">
                {{ row.entity.description || "—" }}
              </td>

              <td class="px-2 py-2.5 text-xs">
                <template v-if="row.firstChapter">
                  <div class="text-slate-300">首次 · 第 {{ row.firstChapter.chapter_index + 1 }} 章</div>
                  <div
                    v-if="chapterHeading(row.firstChapter, book?.title)"
                    class="truncate text-[11px] text-slate-500"
                    :title="chapterHeading(row.firstChapter, book?.title)"
                  >
                    {{ chapterHeading(row.firstChapter, book?.title) }}
                  </div>
                  <div v-if="row.lastChapter && row.lastChapter.chapter_index !== row.firstChapter.chapter_index" class="mt-1 text-slate-400">
                    最近 · 第 {{ row.lastChapter.chapter_index + 1 }} 章
                    <span class="text-slate-600">/ 跨度 {{ row.span }} 章</span>
                  </div>
                </template>
                <div v-else class="text-slate-600">—</div>
                <div class="mt-0.5 text-[10px] text-slate-600">
                  出场 {{ row.eventCount }} 次 / {{ row.chapterCount }} 章
                </div>
              </td>

              <td class="px-2 py-2.5 text-xs">
                <template v-if="row.firstChapter">
                  <div class="text-gold-400">
                    <i class="fa-solid fa-location-dot mr-1" />{{ row.firstPlace || "未标注空间" }}
                  </div>
                  <div v-if="row.firstWhere" class="text-[11px] text-slate-500">{{ row.firstWhere }}</div>
                  <div v-if="row.firstChapter.temporal_type" class="mt-0.5 text-[11px] text-jade-400">
                    {{ row.firstChapter.temporal_type }}
                    <template v-if="hasStoryTime && row.firstDay != null"> · 故事第 {{ row.firstDay }} 天</template>
                  </div>
                </template>
                <span v-else class="text-slate-600">—</span>
              </td>

              <td class="px-2 py-2.5">
                <div v-if="row.topPlaces.length" class="flex flex-wrap gap-1">
                  <span
                    v-for="place in row.topPlaces"
                    :key="place.name"
                    class="rounded border border-ink-600 px-1.5 py-0.5 text-[10px] text-slate-300"
                    :title="`在 ${place.name} 出现 ${place.chapters} 章`"
                  >
                    <i class="fa-solid fa-location-dot mr-1 text-[9px] text-gold-400" />{{ place.name }}
                    <span class="ml-0.5 text-slate-600">{{ place.chapters }}</span>
                  </span>
                </div>
                <span v-else class="text-xs text-slate-600">—</span>
              </td>

              <td class="px-2 py-2.5">
                <div v-if="row.relations.length">
                  <div class="mb-1 text-[10px] text-slate-600">共 {{ row.relations.length }} 段关系</div>
                  <div class="flex flex-wrap gap-1">
                    <button
                      v-for="rel in row.relations.slice(0, 6)"
                      :key="rel.id"
                      class="rounded-full border border-jade-500/30 bg-jade-500/5 px-2 py-0.5 text-[10px] text-jade-300 transition hover:border-jade-400 hover:bg-jade-500/15"
                      :title="`与 ${rel.name} 共同出场 ${rel.weight} 章（第 ${rel.first + 1}–${rel.last + 1} 章） · 我：${rel.selfRole || '—'} / 对方：${rel.otherRole || '—'}`"
                      @click="query = rel.name"
                    >
                      {{ rel.name }}<span class="ml-0.5 text-slate-500">{{ rel.weight }}</span>
                    </button>
                    <span v-if="row.relations.length > 6" class="self-center text-[10px] text-slate-500">
                      +{{ row.relations.length - 6 }}
                    </span>
                  </div>
                </div>
                <span v-else class="text-xs text-slate-600">—</span>
              </td>
            </tr>
          </tbody>
        </table>

        <p v-if="!rows.length" class="py-10 text-center text-sm text-slate-500">没有匹配的人物。</p>
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
