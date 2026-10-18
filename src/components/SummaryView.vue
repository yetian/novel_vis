<script setup lang="ts">
import { computed, ref } from "vue";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import { api, isTauri } from "@/api/tauri";
import { useLibraryStore } from "@/stores/library";
import { useLlmStore } from "@/stores/llm";
import { useUiStore } from "@/stores/ui";
import { chapterHeading } from "@/utils/chapter";
import { errText } from "@/utils/error";
import { computeImportance, importanceLabel } from "@/utils/people";
import { cleanPlace, cleanWorld } from "@/utils/space";
import type { ChapterRow, EntityEventRow, EntityRow } from "@/types/schema";

const library = useLibraryStore();
const llm = useLlmStore();
const ui = useUiStore();
const query = ref("");
const generating = ref(false);

const timeline = computed(() => library.timeline);
const chapters = computed(() => timeline.value?.chapters ?? []);
const entities = computed(() => timeline.value?.entities ?? []);
const events = computed(() => timeline.value?.events ?? []);
const book = computed(() => timeline.value?.book ?? library.activeBook);
/// 旧数据可能把 “...” 之类占位符写进 summary，这里视为空，允许重新生成。
const bookSummary = computed(() => {
  const text = (book.value?.summary ?? "").trim();
  if (!text) return "";
  const meaningful = text.replace(/[\s\p{P}\p{S}]/gu, "");
  return meaningful.length >= 20 ? text : "";
});

const characterEntities = computed(() => entities.value.filter((e) => e.entity_type === "CHARACTER"));
const characterIds = computed(() => new Set(characterEntities.value.map((e) => e.id)));
const entityById = computed(() => new Map(entities.value.map((e) => [e.id, e])));

const eventsByChar = computed(() => {
  const m = new Map<number, EntityEventRow[]>();
  for (const ev of events.value) {
    const list = m.get(ev.entity_id) ?? [];
    list.push(ev);
    m.set(ev.entity_id, list);
  }
  return m;
});

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

/// 全书重要度：与「人物关系」页保持同一套公式。
const importanceById = computed(() => {
  const maxEvents = characterEntities.value.reduce(
    (max, e) => Math.max(max, (eventsByChar.value.get(e.id) ?? []).length),
    1,
  );
  const totalChapters = Math.max(1, chapters.value.length);
  const totalSpan = Math.max(1, chapters.value.length - 1);
  const result = new Map<number, number>();
  for (const entity of characterEntities.value) {
    const evs = eventsByChar.value.get(entity.id) ?? [];
    const chapterSet = new Set(evs.map((e) => e.chapter_index));
    let first = entity.first_appeared_chapter_index ?? null;
    let last = first;
    for (const ev of evs) {
      if (first == null || ev.chapter_index < first) first = ev.chapter_index;
      if (last == null || ev.chapter_index > last) last = ev.chapter_index;
    }
    const span = first != null && last != null ? Math.max(0, last - first) : 0;
    const explicit =
      entity.importance != null && entity.importance >= 1 && entity.importance <= 5
        ? Math.round(entity.importance)
        : null;
    result.set(
      entity.id,
      computeImportance({
        explicit,
        coverage: chapterSet.size / totalChapters,
        volume: evs.length / maxEvents,
        spanRatio: span / totalSpan,
        role: roleByChar.value.get(entity.id),
      }),
    );
  }
  return result;
});

const firstByChapter = computed(() => {
  const m = new Map<number, EntityRow[]>();
  for (const e of characterEntities.value) {
    if (e.first_appeared_chapter_index == null) continue;
    const list = m.get(e.first_appeared_chapter_index) ?? [];
    list.push(e);
    m.set(e.first_appeared_chapter_index, list);
  }
  for (const list of m.values()) {
    list.sort(
      (a, b) =>
        (importanceById.value.get(b.id) ?? 1) - (importanceById.value.get(a.id) ?? 1) ||
        a.name.localeCompare(b.name),
    );
  }
  return m;
});

/// 本章主要人物：按全书重要度 + 本章事件数排序；首次登场者单列，避免重复。
const mainByChapter = computed(() => {
  const byChapter = new Map<number, Map<number, number>>();
  for (const ev of events.value) {
    if (!characterIds.value.has(ev.entity_id)) continue;
    let m = byChapter.get(ev.chapter_index);
    if (!m) {
      m = new Map<number, number>();
      byChapter.set(ev.chapter_index, m);
    }
    m.set(ev.entity_id, (m.get(ev.entity_id) ?? 0) + 1);
  }

  const result = new Map<
    number,
    { entity: EntityRow; importance: number; role: string; count: number }[]
  >();
  for (const c of chapters.value) {
    const counts = byChapter.get(c.chapter_index);
    if (!counts?.size) continue;
    const firstIds = new Set(
      (firstByChapter.value.get(c.chapter_index) ?? []).map((e) => e.id),
    );
    const list = [...counts.entries()]
      .map(([id, count]) => {
        const entity = entityById.value.get(id);
        return entity
          ? {
              entity,
              importance: importanceById.value.get(id) ?? 1,
              role: roleByChar.value.get(id) ?? "",
              count,
            }
          : null;
      })
      .filter((x): x is { entity: EntityRow; importance: number; role: string; count: number } => !!x)
      .filter((x) => !firstIds.has(x.entity.id))
      .sort(
        (a, b) =>
          b.importance - a.importance ||
          b.count - a.count ||
          a.entity.name.localeCompare(b.entity.name),
      )
      .slice(0, 6);
    if (list.length) result.set(c.chapter_index, list);
  }
  return result;
});

const stats = computed(() => {
  const summarized = chapters.value.filter((c) => (c.raw_text_summary || "").trim()).length;
  const locations = new Set(
    chapters.value.map((c) => `${c.world || ""}::${c.region || ""}::${c.site || ""}`),
  );
  return {
    summarized,
    characters: characterEntities.value.length,
    items: entities.value.filter((e) => e.entity_type !== "CHARACTER").length,
    locations: locations.size,
  };
});

const allExtracted = computed(() => {
  const b = book.value;
  return !!b && b.total_chapters > 0 && (b.extracted_chapters ?? 0) >= b.total_chapters;
});

const rows = computed(() => {
  const q = query.value.trim();
  if (!q) return chapters.value;
  return chapters.value.filter(
    (c) =>
      [
        c.title,
        c.raw_text_summary,
        c.world,
        c.region,
        c.site,
        ...(mainByChapter.value.get(c.chapter_index) ?? []).map((p) => p.entity.name),
        ...(firstByChapter.value.get(c.chapter_index) ?? []).map((p) => p.name),
      ].some((v) => (v || "").includes(q)),
  );
});

const firstList = (chapterIndex: number) => firstByChapter.value.get(chapterIndex) ?? [];

const chapterHeadingOf = (c: ChapterRow) => chapterHeading(c, book.value?.title);

const locationText = (c: ChapterRow) =>
  [cleanPlace(c.site) || cleanPlace(c.region) || cleanWorld(c.world), cleanWorld(c.world)]
    .filter(Boolean)
    .filter((v, i, arr) => arr.indexOf(v) === i)
    .join(" · ");

const importanceChipClass = (n: number) => {
  if (n >= 5) return "border-gold-400/60 bg-gold-400/10 text-gold-300";
  if (n >= 4) return "border-jade-500/50 bg-jade-500/10 text-jade-300";
  if (n >= 3) return "border-sky-400/40 bg-sky-400/5 text-sky-300";
  return "border-ink-500 bg-ink-900/40 text-slate-300";
};

async function generateSummary() {
  if (!book.value || generating.value) return;
  if (!llm.config.model) {
    ui.toast("请先在「引擎设置」中选择模型", "error");
    return;
  }
  generating.value = true;
  try {
    await api.generateBookSummary(book.value.id, llm.config);
    await library.reloadTimeline();
    ui.toast("全书总览已生成", "success");
  } catch (e) {
    ui.toast(`全书总览生成失败：${errText(e)}`, "error");
  } finally {
    generating.value = false;
  }
}

function summaryPayload() {
  return {
    book_id: book.value?.id ?? null,
    title: book.value?.title ?? "",
    author: book.value?.author ?? null,
    summary: bookSummary.value,
  };
}

function csvCell(value: unknown): string {
  const text = value == null ? "" : String(value);
  return /[;"\r\n]/.test(text) ? `"${text.replace(/"/g, '""')}"` : text;
}

function summaryCsv(): string {
  const payload = summaryPayload();
  const header = ["book_id", "title", "author", "summary"].join(";");
  const row = [payload.book_id, payload.title, payload.author, payload.summary]
    .map(csvCell)
    .join(";");
  return `\uFEFF${header}\r\n${row}\r\n`;
}

async function exportSummary(format: "json" | "csv") {
  if (!bookSummary.value) return;
  if (!isTauri) {
    ui.toast("导出功能需要在桌面端中使用", "error");
    return;
  }
  try {
    const title = (book.value?.title || "book")
      .replace(/[\\/:*?"<>|]+/g, "_")
      .slice(0, 80);
    const target = await save({
      defaultPath: `${title}-全书总览.${format}`,
      filters: [{ name: format === "json" ? "JSON" : "CSV", extensions: [format] }],
    });
    if (!target) return;
    const content =
      format === "json" ? JSON.stringify(summaryPayload(), null, 2) : summaryCsv();
    await writeTextFile(target as string, content);
    ui.toast(`已导出 ${format.toUpperCase()} 文件`, "success");
  } catch (e) {
    ui.toast(`导出失败：${errText(e)}`, "error");
  }
}
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
          <input v-model="query" class="input max-w-xs !py-1.5 text-xs" placeholder="搜索章节 / 摘要 / 人物 / 地点…" />
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

      <!-- 全书总览：有内容时只展示正文 -->
      <section class="panel group relative p-5">
        <template v-if="bookSummary">
          <p class="whitespace-pre-line text-sm leading-relaxed text-slate-300">
            {{ bookSummary }}
          </p>
          <div
            class="absolute right-3 top-3 flex items-center gap-1 rounded-lg border border-ink-500/40 bg-ink-800/95 p-0.5 shadow-sm backdrop-blur transition-opacity"
            :class="
              generating
                ? 'opacity-100'
                : 'pointer-events-none opacity-0 group-hover:pointer-events-auto group-hover:opacity-100 focus-within:pointer-events-auto focus-within:opacity-100'
            "
          >
            <button
              class="btn-ghost !p-1.5"
              title="导出 JSON"
              aria-label="导出全书总览 JSON"
              @click="exportSummary('json')"
            >
              <i class="fa-solid fa-file-code text-xs" />
            </button>
            <button
              class="btn-ghost !p-1.5"
              title="导出 CSV（分号分隔）"
              aria-label="导出全书总览 CSV"
              @click="exportSummary('csv')"
            >
              <i class="fa-solid fa-file-csv text-xs" />
            </button>
            <button
              v-if="allExtracted"
              class="btn-ghost !p-1.5"
              :disabled="generating"
              title="重新生成全书总览"
              aria-label="重新生成全书总览"
              @click="generateSummary"
            >
              <i class="fa-solid fa-rotate text-xs" :class="{ 'fa-spin': generating }" />
            </button>
          </div>
        </template>

        <template v-else>
          <div class="flex flex-wrap items-center gap-2">
            <h3 class="text-sm font-semibold text-slate-100">
              <i class="fa-solid fa-book-open mr-1 text-gold-400" />全书总览
            </h3>
            <button
              v-if="allExtracted && !generating"
              class="btn-ghost ml-auto !px-2 !py-1 text-xs"
              @click="generateSummary"
            >
              <i class="fa-solid fa-wand-magic-sparkles" />生成全书总览
            </button>
            <span v-else-if="generating" class="ml-auto text-xs text-gold-400">
              <i class="fa-solid fa-spinner fa-spin mr-1" />生成中…
            </span>
          </div>

          <p v-if="book?.summary" class="mt-2 text-sm text-slate-500">
            现有总览内容无效（可能只生成了占位符），请点击右上角生成新的总览。
          </p>
          <p v-else-if="allExtracted" class="mt-2 text-sm text-slate-500">
            抽取已完成，可点击右上角生成全书总览。
          </p>
          <p v-else class="mt-2 text-sm text-slate-500">
            完成全书抽取后，将自动汇总主线、关键人物与世界/空间信息。
          </p>
        </template>
      </section>

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
            <h3
              v-if="chapterHeadingOf(c)"
              class="min-w-0 truncate text-sm font-semibold text-slate-100"
            >
              {{ chapterHeadingOf(c) }}
            </h3>
            <span
              v-if="locationText(c)"
              class="rounded-full border border-gold-400/40 px-2 py-0.5 text-[10px] text-gold-400"
            >
              <i class="fa-solid fa-location-dot mr-1" />{{ locationText(c) }}
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

          <div class="mt-2 space-y-1.5">
            <div
              v-if="mainByChapter.get(c.chapter_index)?.length"
              class="flex flex-wrap items-center gap-1"
            >
              <span class="text-[11px] text-slate-500">本章主要人物</span>
              <span
                v-for="p in mainByChapter.get(c.chapter_index)"
                :key="p.entity.id"
                class="rounded-full border px-2 py-0.5 text-[10px]"
                :class="importanceChipClass(p.importance)"
                :title="`${importanceLabel(p.importance)}${p.role ? ` · ${p.role}` : ''} · 本章 ${p.count} 次事件`"
              >
                {{ p.entity.name }}
              </span>
            </div>

            <div
              v-if="firstList(c.chapter_index).length"
              class="flex flex-wrap items-center gap-1"
            >
              <span class="text-[11px] text-slate-500">首次登场</span>
              <span
                v-for="p in firstList(c.chapter_index).slice(0, 12)"
                :key="p.id"
                class="rounded-full border border-jade-500/40 bg-jade-500/10 px-2 py-0.5 text-[10px] text-jade-300"
                :title="`${importanceLabel(importanceById.get(p.id) ?? 1)}${p.description ? ` · ${p.description}` : ''}`"
              >
                {{ p.name }}
              </span>
              <span
                v-if="firstList(c.chapter_index).length > 12"
                class="self-center text-[10px] text-slate-500"
              >
                +{{ firstList(c.chapter_index).length - 12 }}
              </span>
            </div>
          </div>
        </article>
      </div>
    </div>
  </div>
</template>
