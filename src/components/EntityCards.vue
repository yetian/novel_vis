<script setup lang="ts">
import { computed, ref } from "vue";
import { useLibraryStore } from "@/stores/library";

const library = useLibraryStore();
const tab = ref<"spaces" | "things">("spaces");
const query = ref("");

const timeline = computed(() => library.timeline);
const chapters = computed(() => timeline.value?.chapters ?? []);
const entities = computed(() => timeline.value?.entities ?? []);
const events = computed(() => timeline.value?.events ?? []);

const entityById = computed(() => new Map(entities.value.map((e) => [e.id, e])));
const worldOf = (w?: string | null) => w || "未知空间";
const siteOf = (c: { world?: string | null; region?: string | null; site?: string | null }) =>
  c.site || c.region || worldOf(c.world);
const siteKey = (c: { world?: string | null; region?: string | null; site?: string | null }) =>
  `${worldOf(c.world)}::${siteOf(c)}`;

const spaces = computed(() => {
  const chapterSite = new Map<number, string>();
  const sites = new Map<
    string,
    { key: string; world: string; region: string | null; site: string; chapters: number[]; chars: Set<number>; items: Set<number> }
  >();

  for (const c of chapters.value) {
    const key = siteKey(c);
    chapterSite.set(c.chapter_index, key);
    if (!sites.has(key)) {
      sites.set(key, { key, world: worldOf(c.world), region: c.region ?? null, site: siteOf(c), chapters: [], chars: new Set(), items: new Set() });
    }
    sites.get(key)!.chapters.push(c.chapter_index);
  }

  for (const ev of events.value) {
    const ent = entityById.value.get(ev.entity_id);
    if (!ent) continue;
    const key = chapterSite.get(ev.chapter_index);
    if (key) {
      const s = sites.get(key)!;
      if (ent.entity_type === "CHARACTER") s.chars.add(ev.entity_id);
      else s.items.add(ev.entity_id);
    }
    if (ev.acquired_site) {
      for (const s of sites.values()) {
        if (ev.acquired_site.includes(s.site)) s.items.add(ev.entity_id);
      }
    }
  }

  return [...sites.values()]
    .map((s) => ({ ...s, chapters: s.chapters.sort((a, b) => a - b) }))
    .sort((a, b) => b.chapters.length - a.chapters.length);
});

const things = computed(() => {
  const nonChar = entities.value.filter((e) => e.entity_type !== "CHARACTER");
  return nonChar
    .map((it) => {
      const evs = events.value.filter((e) => e.entity_id === it.id);
      const acquire = evs.find((e) => e.action_type === "ACQUIRE") ?? evs[0];
      return {
        it,
        count: evs.length,
        site: acquire?.acquired_site ?? null,
        usage: acquire?.impact_detail ?? it.description ?? null,
      };
    })
    .sort((a, b) => b.count - a.count);
});

const filteredSpaces = computed(() =>
  spaces.value.filter((s) => (query.value ? (s.site + s.world + (s.region ?? "")).includes(query.value) : true)),
);
const filteredThings = computed(() =>
  things.value.filter((t) => (query.value ? t.it.name.includes(query.value) : true)),
);

const categoryLabel: Record<string, string> = { ITEM: "物品", SKILL: "功法", LEGACY: "传承" };
const nameOf = (id: number) => entityById.value.get(id)?.name ?? "";
</script>

<template>
  <div class="h-full overflow-y-auto p-6">
    <div v-if="!timeline || !chapters.length" class="grid h-full place-items-center text-center text-slate-500">
      <div>
        <div class="mx-auto mb-4 grid h-16 w-16 place-items-center rounded-2xl bg-ink-700 text-2xl">
          <i class="fa-solid fa-table-cells-large" />
        </div>
        尚无数据。请先抽取该书。
      </div>
    </div>

    <div v-else class="mx-auto max-w-6xl">
      <div class="mb-5 flex flex-wrap items-center gap-3">
        <div class="flex rounded-lg bg-ink-900/60 p-1">
          <button
            class="btn !px-4 !py-1.5"
            :class="tab === 'spaces' ? 'bg-jade-500 text-ink-900' : 'text-slate-300 hover:bg-ink-600'"
            @click="tab = 'spaces'"
          >
            <i class="fa-solid fa-mountain-sun" />空间地点 ({{ spaces.length }})
          </button>
          <button
            class="btn !px-4 !py-1.5"
            :class="tab === 'things' ? 'bg-gold-400 text-ink-900' : 'text-slate-300 hover:bg-ink-600'"
            @click="tab = 'things'"
          >
            <i class="fa-solid fa-gem" />物品功法 ({{ things.length }})
          </button>
        </div>
        <input v-model="query" class="input ml-auto !w-56 !py-1.5 text-xs" placeholder="搜索…" />
      </div>

      <!-- 空间地点 -->
      <div v-if="tab === 'spaces'" class="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
        <article v-for="s in filteredSpaces" :key="s.key" class="panel p-4">
          <div class="flex items-start gap-2">
            <i class="fa-solid fa-location-dot mt-0.5 text-jade-400" />
            <div class="min-w-0 flex-1">
              <h4 class="truncate font-medium text-slate-100">{{ s.site }}</h4>
              <p class="text-[11px] text-slate-500">{{ s.world }}<template v-if="s.region"> · {{ s.region }}</template></p>
            </div>
            <span class="rounded bg-ink-600 px-2 py-0.5 text-[11px] text-slate-400">{{ s.chapters.length }} 章</span>
          </div>

          <div v-if="s.chars.size" class="mt-3">
            <div class="mb-1 text-[11px] uppercase tracking-wide text-slate-500">人物</div>
            <div class="flex flex-wrap gap-1">
              <span v-for="id in [...s.chars].slice(0, 8)" :key="id" class="rounded-full border border-jade-500/40 bg-jade-500/10 px-2 py-0.5 text-[11px] text-jade-300">
                {{ nameOf(id) }}
              </span>
            </div>
          </div>

          <div v-if="s.items.size" class="mt-2">
            <div class="mb-1 text-[11px] uppercase tracking-wide text-slate-500">物品 / 功法</div>
            <div class="flex flex-wrap gap-1">
              <span v-for="id in [...s.items].slice(0, 8)" :key="id" class="rounded-full border border-gold-400/40 bg-gold-400/10 px-2 py-0.5 text-[11px] text-gold-400">
                {{ nameOf(id) }}
              </span>
            </div>
          </div>
        </article>
        <p v-if="!filteredSpaces.length" class="col-span-full py-10 text-center text-sm text-slate-500">无匹配地点。</p>
      </div>

      <!-- 物品功法 -->
      <div v-else class="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
        <article v-for="t in filteredThings" :key="t.it.id" class="panel p-4">
          <div class="flex items-start gap-2">
            <i
              class="fa-solid mt-0.5 text-gold-400"
              :class="t.it.entity_type === 'SKILL' ? 'fa-hand-fist' : 'fa-gem'"
            />
            <div class="min-w-0 flex-1">
              <h4 class="truncate font-medium text-slate-100">{{ t.it.name }}</h4>
              <p class="text-[11px] text-slate-500">
                {{ categoryLabel[t.it.entity_type] ?? t.it.entity_type }}
                <template v-if="t.it.first_appeared_chapter_index != null"> · 首现 第 {{ t.it.first_appeared_chapter_index + 1 }} 章</template>
              </p>
            </div>
            <span class="rounded bg-ink-600 px-2 py-0.5 text-[11px] text-slate-400">{{ t.count }} 次</span>
          </div>
          <p v-if="t.site" class="mt-2 text-[11px] text-slate-400">
            <i class="fa-solid fa-location-dot mr-1" />{{ t.site }}
          </p>
          <p v-if="t.usage" class="mt-1 line-clamp-3 text-[11px] text-slate-500">{{ t.usage }}</p>
        </article>
        <p v-if="!filteredThings.length" class="col-span-full py-10 text-center text-sm text-slate-500">无匹配物品。</p>
      </div>
    </div>
  </div>
</template>
