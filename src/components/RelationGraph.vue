<script setup lang="ts">
import { computed, ref } from "vue";
import { useLibraryStore } from "@/stores/library";
import { cleanPlace, cleanWorld } from "@/utils/space";
import type { ChapterRow, EntityEventRow, EntityRow } from "@/types/schema";

const library = useLibraryStore();
const query = ref("");

const timeline = computed(() => library.timeline);
const chapters = computed(() => timeline.value?.chapters ?? []);
const entities = computed(() => timeline.value?.entities ?? []);
const events = computed(() => timeline.value?.events ?? []);

const normalizeName = (s?: string | null) => (s || "").replace(/[\s·•,，.。:：()（）\-—_]/g, "").toLowerCase();
const bookAuthor = computed(() => (timeline.value?.book.author || "").trim());
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

/// 关系 = 同一章节共同出场；权重为共现章节数。
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

  const weights = new Map<string, { a: number; b: number; weight: number }>();
  for (const ids of byChapter.values()) {
    const list = [...ids].sort((a, b) => a - b);
    for (let i = 0; i < list.length; i++) {
      for (let j = i + 1; j < list.length; j++) {
        const key = `${list[i]}|${list[j]}`;
        const found = weights.get(key);
        if (found) found.weight += 1;
        else weights.set(key, { a: list[i], b: list[j], weight: 1 });
      }
    }
  }

  const nameById = new Map(characters.value.map((c) => [c.id, c.name]));
  const result = new Map<number, { id: number; name: string; weight: number }[]>();
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
    ensure(w.a).push({ id: w.b, name: bn, weight: w.weight });
    ensure(w.b).push({ id: w.a, name: an, weight: w.weight });
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
  firstDay: number | null;
  firstPlace: string;
  firstWhere: string;
  relations: { id: number; name: string; weight: number }[];
}

/// 先汇总全书范围，再用“模型判断 + 覆盖章节 + 出场频率 + 时间跨度”计算全局重要度。
const globalStats = computed(() => {
  const list = characters.value.map((entity) => {
    const evs = eventsByChar.value.get(entity.id) ?? [];
    const chapterSet = new Set(evs.map((e) => e.chapter_index));
    const first =
      entity.first_appeared_chapter_index ??
      (evs.length ? Math.min(...evs.map((e) => e.chapter_index)) : null);
    const last = evs.length ? Math.max(...evs.map((e) => e.chapter_index)) : first;
    return { entity, evs, chapterSet, first, last };
  });
  return {
    list,
    maxEvents: Math.max(1, ...list.map((x) => x.evs.length)),
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
      let raw: number;
      if (explicit != null) {
        // 模型判断占一半权重，其余由全书覆盖/频率/跨度校准。
        raw = 0.5 * explicit + 5 * (0.2 * coverage + 0.15 * volume + 0.15 * spanRatio);
      } else {
        raw = 5 * (0.35 * coverage + 0.35 * volume + 0.3 * spanRatio);
      }
      if (/主角|男主|女主|protagonist/i.test(role)) raw += 0.5;
      else if (/反派|主要|核心|boss/i.test(role)) raw += 0.25;
      const importance = Math.max(1, Math.min(5, Math.ceil(raw)));

      const firstPlace = firstChapter
        ? cleanPlace(firstChapter.site) ||
          cleanPlace(firstChapter.region) ||
          cleanWorld(firstChapter.world) ||
          ""
        : "";
      const firstWhere = firstChapter
        ? [cleanWorld(firstChapter.world), cleanPlace(firstChapter.region)].filter(Boolean).join(" · ")
        : "";

      return {
        entity,
        role,
        importance,
        explicit,
        eventCount,
        chapterCount,
        span,
        firstChapter,
        firstDay:
          firstIndex != null ? dayByChapter.value.get(firstIndex) ?? null : null,
        firstPlace,
        firstWhere,
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

const rows = computed(() => {
  const q = query.value.trim();
  if (!q) return allRows.value;
  return allRows.value.filter(
    (r) =>
      r.entity.name.includes(q) ||
      (r.entity.description || "").includes(q) ||
      r.role.includes(q) ||
      r.relations.some((rel) => rel.name.includes(q)),
  );
});

const IMPORTANCE_LABELS: Record<number, string> = {
  5: "核心主角",
  4: "主要人物",
  3: "重要配角",
  2: "次要人物",
  1: "路人",
};
const importanceLabel = (n: number) => IMPORTANCE_LABELS[n] ?? "—";
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
            共 {{ characters.length }} 人 · 重要度按全书汇总（模型判断 + 覆盖章节 + 出场频率 + 时间跨度）· 已排除作者署名 · 关系 = 同章共同出场
          </p>
        </div>
        <div class="ml-auto flex items-center gap-2">
          <input v-model="query" class="input w-56 !py-1.5 text-xs" placeholder="搜索姓名 / 简介 / 关系…" />
          <button v-if="query" class="btn-ghost !px-2 !py-1 text-xs" title="清除搜索" @click="query = ''">
            <i class="fa-solid fa-xmark" />
          </button>
        </div>
      </header>

      <div class="min-h-0 flex-1 overflow-auto">
        <table class="w-full min-w-[980px] border-collapse text-left text-sm">
          <thead class="sticky top-0 z-10 bg-ink-800/95 text-[11px] uppercase tracking-wide text-slate-500 backdrop-blur">
            <tr>
              <th class="w-12 px-3 py-2 font-medium">#</th>
              <th class="min-w-[130px] px-3 py-2 font-medium">人物</th>
              <th class="w-28 px-3 py-2 font-medium">重要度</th>
              <th class="min-w-[180px] px-3 py-2 font-medium">简介</th>
              <th class="min-w-[160px] px-3 py-2 font-medium">首次出现</th>
              <th class="min-w-[190px] px-3 py-2 font-medium">首现时空</th>
              <th class="min-w-[240px] px-3 py-2 font-medium">关系</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="(row, i) in rows"
              :key="row.entity.id"
              class="border-t border-ink-700/50 align-top transition hover:bg-ink-700/20"
            >
              <td class="px-3 py-2.5 text-xs text-slate-600">{{ i + 1 }}</td>

              <td class="px-3 py-2.5">
                <div class="flex items-center gap-2">
                  <i class="fa-solid fa-user text-[11px] text-jade-400" />
                  <span class="font-medium text-slate-100">{{ row.entity.name }}</span>
                </div>
                <div v-if="row.role" class="mt-0.5 text-[11px] text-jade-400">{{ row.role }}</div>
              </td>

              <td class="px-3 py-2.5">
                <div
                  class="flex items-center gap-0.5"
                  :title="`重要度 ${row.importance}/5${row.explicit != null ? ` · 模型判断 ${row.explicit}/5` : ''} · 出场 ${row.eventCount} 次 / ${row.chapterCount} 章`"
                >
                  <span
                    v-for="n in 5"
                    :key="n"
                    class="h-1.5 w-3 rounded-sm"
                    :class="n <= row.importance ? 'bg-gold-400' : 'bg-ink-600'"
                  />
                </div>
                <div class="mt-1 text-[10px] text-slate-500">{{ importanceLabel(row.importance) }}</div>
              </td>

              <td class="px-3 py-2.5 text-xs leading-relaxed text-slate-400">
                {{ row.entity.description || "—" }}
              </td>

              <td class="px-3 py-2.5 text-xs">
                <template v-if="row.firstChapter">
                  <div class="text-slate-300">第 {{ row.firstChapter.chapter_index + 1 }} 章</div>
                  <div class="truncate text-[11px] text-slate-500" :title="row.firstChapter.title">
                    {{ row.firstChapter.title || "未命名章节" }}
                  </div>
                </template>
                <div v-else class="text-slate-600">—</div>
                <div class="mt-0.5 text-[10px] text-slate-600">
                  出场 {{ row.eventCount }} 次 / {{ row.chapterCount }} 章
                </div>
              </td>

              <td class="px-3 py-2.5 text-xs">
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

              <td class="px-3 py-2.5">
                <div v-if="row.relations.length" class="flex flex-wrap gap-1">
                  <button
                    v-for="rel in row.relations.slice(0, 5)"
                    :key="rel.id"
                    class="rounded-full border border-jade-500/30 bg-jade-500/5 px-2 py-0.5 text-[10px] text-jade-300 transition hover:border-jade-400 hover:bg-jade-500/15"
                    :title="`与 ${rel.name} 共同出场 ${rel.weight} 章`"
                    @click="query = rel.name"
                  >
                    {{ rel.name }}<span class="ml-0.5 text-slate-500">{{ rel.weight }}</span>
                  </button>
                  <span v-if="row.relations.length > 5" class="self-center text-[10px] text-slate-500">
                    +{{ row.relations.length - 5 }}
                  </span>
                </div>
                <span v-else class="text-xs text-slate-600">—</span>
              </td>
            </tr>
          </tbody>
        </table>

        <p v-if="!rows.length" class="py-10 text-center text-sm text-slate-500">没有匹配的人物。</p>
      </div>
    </div>
  </div>
</template>
