<script setup lang="ts">
import { computed, ref } from "vue";
import { useLibraryStore } from "@/stores/library";

const library = useLibraryStore();
const query = ref("");

const timeline = computed(() => library.timeline);
const chapters = computed(() => timeline.value?.chapters ?? []);
const entities = computed(() => timeline.value?.entities ?? []);
const book = computed(() => timeline.value?.book ?? library.activeBook);

const stats = computed(() => {
  const summarized = chapters.value.filter((c) => (c.raw_text_summary || "").trim()).length;
  const locations = new Set(
    chapters.value.map((c) => `${c.world || ""}::${c.region || ""}::${c.site || ""}`),
  );
  return {
    summarized,
    characters: entities.value.filter((e) => e.entity_type === "CHARACTER").length,
    items: entities.value.filter((e) => e.entity_type !== "CHARACTER").length,
    locations: locations.size,
  };
});

const firstAppearByChapter = computed(() => {
  const m = new Map<number, string[]>();
  for (const e of entities.value) {
    if (e.entity_type !== "CHARACTER" || e.first_appeared_chapter_index == null) continue;
    const list = m.get(e.first_appeared_chapter_index) ?? [];
    list.push(e.name);
    m.set(e.first_appeared_chapter_index, list);
  }
  return m;
});

const rows = computed(() => {
  const q = query.value.trim();
  if (!q) return chapters.value;
  return chapters.value.filter((c) =>
    [c.title, c.raw_text_summary, c.world, c.region, c.site].some((v) => (v || "").includes(q)),
  );
});
</script>

<template>
  <div class="h-full overflow-y-auto p-5">
    <div v-if="!timeline || !chapters.length" class="grid h-full place-items-center p-8">
      <div class="max-w-md text-center">
        <div class="mx-auto mb-4 grid h-16 w-16 place-items-center rounded-2xl bg-ink-700 text-2xl text-slate-500">
          <i class="fa-solid fa-list-ul" />
        </div>
        <h2 class="text-lg font-semibold text-slate-200">暂无章节摘要</h2>
        <p class="mt-2 text-sm text-slate-500">
          请先在「图书馆」导入电子书并运行批量抽取，完成后即可在此按章查看剧情摘要与首现人物。
        </p>
      </div>
    </div>

    <div v-else class="mx-auto max-w-4xl space-y-4">
      <header class="panel p-5">
        <div class="flex flex-wrap items-start gap-3">
          <div class="min-w-0 flex-1">
            <h2 class="truncate text-lg font-semibold text-slate-100">{{ book?.title || "未命名书籍" }}</h2>
            <p class="text-xs text-slate-500">
              <template v-if="book?.author">{{ book.author }} · </template>
              共 {{ chapters.length }} 章 · 已生成摘要 {{ stats.summarized }} 章
            </p>
          </div>
          <input v-model="query" class="input max-w-xs !py-1.5 text-xs" placeholder="搜索章节 / 摘要 / 地点…" />
        </div>

        <div class="mt-4 grid grid-cols-2 gap-2 sm:grid-cols-5">
          <div class="rounded-lg border border-ink-600/50 bg-ink-900/40 px-3 py-2">
            <div class="text-[10px] uppercase tracking-wide text-slate-500">章节</div>
            <div class="text-lg font-semibold text-slate-100">{{ chapters.length }}</div>
          </div>
          <div class="rounded-lg border border-ink-600/50 bg-ink-900/40 px-3 py-2">
            <div class="text-[10px] uppercase tracking-wide text-slate-500">摘要</div>
            <div class="text-lg font-semibold text-jade-400">{{ stats.summarized }}</div>
          </div>
          <div class="rounded-lg border border-ink-600/50 bg-ink-900/40 px-3 py-2">
            <div class="text-[10px] uppercase tracking-wide text-slate-500">人物</div>
            <div class="text-lg font-semibold text-gold-400">{{ stats.characters }}</div>
          </div>
          <div class="rounded-lg border border-ink-600/50 bg-ink-900/40 px-3 py-2">
            <div class="text-[10px] uppercase tracking-wide text-slate-500">物品 / 功法</div>
            <div class="text-lg font-semibold text-gold-400">{{ stats.items }}</div>
          </div>
          <div class="rounded-lg border border-ink-600/50 bg-ink-900/40 px-3 py-2">
            <div class="text-[10px] uppercase tracking-wide text-slate-500">地点</div>
            <div class="text-lg font-semibold text-jade-400">{{ stats.locations }}</div>
          </div>
        </div>
      </header>

      <div class="space-y-2">
        <p v-if="!rows.length" class="panel px-4 py-6 text-center text-sm text-slate-500">没有匹配的章节。</p>

        <article
          v-for="c in rows"
          :key="c.id"
          class="panel px-4 py-3 transition hover:border-ink-500"
        >
          <div class="flex flex-wrap items-center gap-2">
            <span
              class="rounded-md border border-ink-500 bg-ink-900/60 px-1.5 py-0.5 text-[11px] font-semibold text-slate-400"
            >
              第 {{ c.chapter_index + 1 }} 章
            </span>
            <h3 class="min-w-0 truncate text-sm font-semibold text-slate-100">{{ c.title || "未命名章节" }}</h3>
            <span
              v-if="c.world || c.site"
              class="rounded-full border border-gold-400/40 px-2 py-0.5 text-[10px] text-gold-400"
            >
              <i class="fa-solid fa-location-dot mr-1" />{{ c.site || c.region || c.world
              }}<template v-if="c.world && c.site"> · {{ c.world }}</template>
            </span>
            <span
              v-if="c.temporal_type && c.temporal_type !== 'CONTINUOUS' && c.temporal_type !== 'UNCERTAIN'"
              class="rounded-full border border-jade-500/40 px-2 py-0.5 text-[10px] text-jade-400"
            >
              {{ c.temporal_type }}<template v-if="c.time_offset_days"> · +{{ c.time_offset_days }} 天</template>
            </span>
            <span
              v-if="c.movement_type && c.movement_type !== 'STAY'"
              class="rounded-full border border-ink-500 px-2 py-0.5 text-[10px] text-slate-400"
            >
              {{ c.movement_type }}
            </span>
          </div>

          <p class="mt-2 text-sm leading-relaxed text-slate-300">
            {{ c.raw_text_summary || "（尚未生成摘要）" }}
          </p>

          <div
            v-if="firstAppearByChapter.get(c.chapter_index)?.length"
            class="mt-2 flex flex-wrap items-center gap-1"
          >
            <span class="text-[11px] text-slate-500">首次登场</span>
            <span
              v-for="name in firstAppearByChapter.get(c.chapter_index)"
              :key="name"
              class="rounded-full border border-jade-500/40 bg-jade-500/10 px-2 py-0.5 text-[10px] text-jade-300"
            >
              {{ name }}
            </span>
          </div>
        </article>
      </div>
    </div>
  </div>
</template>
