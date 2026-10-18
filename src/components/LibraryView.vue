<script setup lang="ts">
import { computed } from "vue";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useLibraryStore } from "@/stores/library";
import { useUiStore } from "@/stores/ui";
import { useExtractionStore } from "@/stores/extraction";
import { buildStaticHtml } from "@/exporter/staticHtml";
import { api, isTauri } from "@/api/tauri";
import { errText } from "@/utils/error";
import ExtractionRunner from "@/components/ExtractionRunner.vue";
import type { BookRow } from "@/types/schema";

const library = useLibraryStore();
const ui = useUiStore();
const extraction = useExtractionStore();

const books = computed(() => library.books);

async function pickFiles() {
  if (!isTauri) {
    ui.toast("请在 Tauri 桌面端中导入文件", "error");
    return;
  }
  const selected = await open({
    multiple: true,
    filters: [{ name: "电子书", extensions: ["epub", "mobi", "azw3"] }],
  });
  const list = Array.isArray(selected) ? selected : selected ? [selected] : [];
  for (const path of list) {
    await library.requestIngest(path as string);
  }
}

async function exportHtml(bookId: number) {
  if (!isTauri) {
    ui.toast("导出功能需要在桌面端中使用", "error");
    return;
  }
  try {
    const timeline =
      library.activeBookId === bookId && library.timeline
        ? library.timeline
        : await api.getTimeline(bookId);
    if (!timeline.chapters.length) {
      ui.toast("请先抽取该书的时空数据再导出", "error");
      return;
    }
    const target = await save({
      defaultPath: `${timeline.book.title}-时空编年史.html`,
      filters: [{ name: "HTML", extensions: ["html"] }],
    });
    if (!target) return;
    await api.writeTextFile(target as string, buildStaticHtml(timeline));
    ui.toast("已导出可交互单页 HTML", "success");
  } catch (e) {
    ui.toast(`导出失败：${errText(e)}`, "error");
  }
}

const totalExtracted = computed(() => books.value.reduce((s, b) => s + b.extracted_chapters, 0));
const completedBooks = computed(
  () => books.value.filter((b) => b.total_chapters > 0 && b.extracted_chapters >= b.total_chapters).length,
);
const progressOf = (book: BookRow) =>
  book.total_chapters ? Math.round((book.extracted_chapters / book.total_chapters) * 100) : 0;
const isExtracting = (id: number) => extraction.runningBookIds.includes(id);

function statusOf(book: BookRow): { label: string; class: string } {
  if (isExtracting(book.id)) {
    return { label: "抽取中", class: "border-gold-400/30 bg-gold-400/10 text-gold-400" };
  }
  if (book.total_chapters > 0 && book.extracted_chapters >= book.total_chapters) {
    return { label: "已完成", class: "border-jade-500/30 bg-jade-500/10 text-jade-300" };
  }
  if (book.extracted_chapters > 0) {
    return { label: "继续抽取", class: "border-gold-400/25 bg-gold-400/[0.06] text-gold-400" };
  }
  return { label: "未抽取", class: "border-ink-500/60 bg-ink-700/60 text-slate-400" };
}
</script>

<template>
  <div class="h-full overflow-y-auto p-5 md:p-7">
    <div class="mx-auto max-w-6xl space-y-6">
      <!-- 导入区 -->
      <section class="panel relative overflow-hidden px-6 py-6 md:px-8 md:py-7">
        <div class="pointer-events-none absolute inset-0">
          <div class="absolute -right-20 -top-28 h-64 w-64 rounded-full bg-jade-500/10 blur-3xl" />
          <div class="absolute -bottom-24 right-32 h-48 w-48 rounded-full bg-gold-400/[0.07] blur-3xl" />
        </div>

        <div class="relative flex flex-col gap-6 md:flex-row md:items-center">
          <div
            class="grid h-14 w-14 shrink-0 place-items-center rounded-2xl border border-jade-500/25 bg-jade-500/10 text-2xl text-jade-400 shadow-glow"
          >
            <i class="fa-solid fa-book-open-reader" />
          </div>

          <div class="min-w-0 flex-1">
            <p class="text-[11px] font-semibold uppercase tracking-[0.22em] text-jade-400">本地书库</p>
            <h2 class="mt-1.5 font-serif text-2xl font-semibold leading-snug text-slate-100 md:text-[28px]">
              让故事沿着时间展开
            </h2>
            <p class="mt-2 max-w-2xl text-sm leading-relaxed text-slate-400">
              导入 EPUB / MOBI / AZW3，抽取人物、地点与物品的时空轨迹。全部解析在本地完成，原文仅驻留内存。
            </p>
          </div>

          <div class="flex shrink-0 flex-col items-stretch gap-2 md:items-end">
            <button class="btn-primary px-5 py-2.5" :disabled="library.ingesting" @click="pickFiles">
              <i class="fa-solid fa-file-import" />
              {{ library.ingesting ? "导入中…" : "导入电子书" }}
            </button>
            <span class="text-center text-xs text-slate-500 md:text-right">
              <i class="fa-solid fa-arrow-down-long mr-1 text-jade-500" />也可直接拖入窗口
            </span>
          </div>
        </div>
      </section>

      <!-- 统计 -->
      <section class="grid grid-cols-2 gap-3.5 md:grid-cols-4">
        <div class="panel flex items-center gap-3.5 p-4">
          <div class="grid h-10 w-10 place-items-center rounded-xl bg-jade-500/10 text-jade-400">
            <i class="fa-solid fa-book-open" />
          </div>
          <div>
            <div class="text-2xl font-semibold leading-none tabular-nums text-slate-100">{{ books.length }}</div>
            <div class="mt-1.5 text-xs text-slate-400">藏书数量</div>
          </div>
        </div>

        <div class="panel flex items-center gap-3.5 p-4">
          <div class="grid h-10 w-10 place-items-center rounded-xl bg-gold-400/10 text-gold-400">
            <i class="fa-solid fa-layer-group" />
          </div>
          <div>
            <div class="text-2xl font-semibold leading-none tabular-nums text-slate-100">
              {{ library.totalChapters }}
            </div>
            <div class="mt-1.5 text-xs text-slate-400">总章节数</div>
          </div>
        </div>

        <div class="panel flex items-center gap-3.5 p-4">
          <div class="grid h-10 w-10 place-items-center rounded-xl bg-jade-500/10 text-jade-400">
            <i class="fa-solid fa-wand-magic-sparkles" />
          </div>
          <div>
            <div class="text-2xl font-semibold leading-none tabular-nums text-slate-100">{{ totalExtracted }}</div>
            <div class="mt-1.5 text-xs text-slate-400">已抽取章节</div>
          </div>
        </div>

        <div class="panel flex items-center gap-3.5 p-4">
          <div class="grid h-10 w-10 place-items-center rounded-xl bg-gold-400/10 text-gold-400">
            <i class="fa-solid fa-circle-check" />
          </div>
          <div>
            <div class="text-2xl font-semibold leading-none tabular-nums text-slate-100">{{ completedBooks }}</div>
            <div class="mt-1.5 text-xs text-slate-400">已完成书籍</div>
          </div>
        </div>
      </section>

      <!-- 抽取运行器 -->
      <ExtractionRunner v-if="library.activeBookId" />

      <!-- 书库列表 -->
      <section>
        <div class="mb-3.5 flex items-end justify-between gap-4">
          <div>
            <h3 class="flex items-center gap-2 text-sm font-semibold tracking-wide text-slate-200">
              <i class="fa-solid fa-books text-jade-500" />
              全部藏书
            </h3>
            <p class="mt-1 text-xs text-slate-400">点击书籍进入总览，或从卡片直接开始抽取。</p>
          </div>
          <span class="rounded-full border border-ink-500/50 bg-ink-800/60 px-2.5 py-1 text-[11px] tabular-nums text-slate-400">
            {{ books.length }} 本
          </span>
        </div>

        <div v-if="!books.length" class="panel grid place-items-center px-6 py-16 text-center">
          <div>
            <div class="mx-auto mb-4 grid h-16 w-16 place-items-center rounded-2xl bg-ink-700 text-2xl text-slate-500">
              <i class="fa-regular fa-folder-open" />
            </div>
            <p class="text-base font-medium text-slate-300">还没有藏书</p>
            <p class="mt-1.5 text-sm text-slate-400">从上方导入第一本电子书，开始构建时空档案。</p>
          </div>
        </div>

        <div v-else class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          <article
            v-for="book in books"
            :key="book.id"
            class="group panel relative flex cursor-pointer flex-col overflow-hidden p-5 transition duration-200 hover:-translate-y-0.5 hover:border-ink-500/80 hover:bg-ink-800"
            :class="{ 'ring-1 ring-jade-500/50': book.id === library.activeBookId }"
            @click="library.open(book.id, 'summary')"
          >
            <div
              class="pointer-events-none absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-jade-500/35 to-transparent opacity-0 transition-opacity group-hover:opacity-100"
              :class="{ 'opacity-100': book.id === library.activeBookId }"
            />

            <div class="flex items-start gap-4">
              <div
                class="relative grid h-16 w-12 shrink-0 place-items-center overflow-hidden rounded-lg border border-ink-500/50 bg-gradient-to-br from-ink-600 via-ink-700 to-ink-800 text-jade-400 shadow-md"
              >
                <i class="fa-solid text-lg" :class="book.format === 'AZW3' ? 'fa-book-bookmark' : 'fa-book'" />
                <span class="absolute inset-x-0 bottom-0 bg-ink-900/70 py-0.5 text-center text-[9px] font-medium tracking-wider text-slate-400">
                  {{ book.format }}
                </span>
              </div>

              <div class="min-w-0 flex-1">
                <div class="flex items-start gap-2">
                  <h4
                    class="min-w-0 flex-1 font-serif text-[17px] font-medium leading-snug text-slate-100 line-clamp-2"
                    :title="book.title"
                  >
                    {{ book.title }}
                  </h4>
                  <span
                    class="shrink-0 rounded-full border px-2 py-0.5 text-[10px] font-medium"
                    :class="statusOf(book).class"
                  >
                    {{ statusOf(book).label }}
                  </span>
                </div>

                <p class="mt-1.5 flex items-center gap-1.5 text-xs text-slate-400">
                  <i class="fa-regular fa-user text-[10px] text-slate-500" />
                  <span class="truncate">{{ book.author || "佚名" }}</span>
                  <span class="text-slate-600">·</span>
                  <span class="shrink-0 tabular-nums">{{ book.total_chapters }} 章</span>
                </p>
              </div>
            </div>

            <div class="mt-5">
              <div class="flex items-center justify-between text-xs">
                <span class="text-slate-400">抽取进度</span>
                <span class="font-medium tabular-nums text-slate-300">
                  {{ book.extracted_chapters }} / {{ book.total_chapters }}
                  <span class="ml-1 text-slate-500">{{ progressOf(book) }}%</span>
                </span>
              </div>
              <div class="mt-2 h-1.5 overflow-hidden rounded-full bg-ink-600">
                <div
                  class="h-full rounded-full bg-gradient-to-r from-jade-600 to-jade-400 transition-all duration-500"
                  :style="{ width: progressOf(book) + '%' }"
                />
              </div>
            </div>

            <div class="mt-4 flex items-center gap-2 border-t border-ink-500/40 pt-4">
              <button class="btn-ghost flex-1 !py-1.5 text-xs" @click.stop="library.open(book.id, 'summary')">
                <i class="fa-solid fa-list-ul" />总览
              </button>
              <button class="btn-primary flex-1 !py-1.5 text-xs" @click.stop="library.open(book.id, 'library')">
                <i class="fa-solid fa-wand-magic-sparkles" />抽取
              </button>
              <button
                class="btn-ghost !px-2.5 !py-1.5"
                title="打开时空画布"
                aria-label="打开时空画布"
                @click.stop="library.open(book.id, 'canvas')"
              >
                <i class="fa-solid fa-diagram-project" />
              </button>
              <button
                class="btn-ghost !px-2.5 !py-1.5"
                title="导出 HTML"
                aria-label="导出 HTML"
                @click.stop="exportHtml(book.id)"
              >
                <i class="fa-solid fa-file-export" />
              </button>
              <button
                class="btn-ghost !px-2.5 !py-1.5 text-cinnabar-400 hover:bg-cinnabar-500/15"
                title="删除书籍"
                aria-label="删除书籍"
                @click.stop="library.remove(book.id)"
              >
                <i class="fa-solid fa-trash" />
              </button>
            </div>
          </article>
        </div>
      </section>
    </div>
  </div>
</template>
