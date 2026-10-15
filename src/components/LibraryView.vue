<script setup lang="ts">
import { computed } from "vue";
import { open, save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import { useLibraryStore } from "@/stores/library";
import { useUiStore } from "@/stores/ui";
import { buildStaticHtml } from "@/exporter/staticHtml";
import { api, isTauri } from "@/api/tauri";
import { errText } from "@/utils/error";
import ExtractionRunner from "@/components/ExtractionRunner.vue";

const library = useLibraryStore();
const ui = useUiStore();

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
    await writeTextFile(target as string, buildStaticHtml(timeline));
    ui.toast("已导出可交互单页 HTML", "success");
  } catch (e) {
    ui.toast(`导出失败：${errText(e)}`, "error");
  }
}

const totalExtracted = computed(() => books.value.reduce((s, b) => s + b.extracted_chapters, 0));
</script>

<template>
  <div class="h-full overflow-y-auto p-6">
    <div class="mx-auto max-w-6xl space-y-6">
      <!-- 导入区 -->
      <section class="panel relative overflow-hidden p-8">
        <div class="absolute inset-0 bg-[radial-gradient(400px_200px_at_90%_-20%,rgba(240,180,41,.12),transparent)]" />
        <div class="relative flex flex-col items-center gap-4 text-center">
          <div class="grid h-16 w-16 place-items-center rounded-2xl bg-jade-500/15 text-2xl text-jade-400">
            <i class="fa-solid fa-cloud-arrow-down" />
          </div>
          <div>
            <h2 class="text-xl font-semibold text-slate-100">导入电子书</h2>
            <p class="mt-1 text-sm text-slate-400">
              将 EPUB / MOBI / AZW3 文件拖入窗口，或点击下方按钮选择。原文解析后仅驻留内存。
            </p>
          </div>
          <button class="btn-primary" :disabled="library.ingesting" @click="pickFiles">
            <i class="fa-solid fa-file-import" :class="{ 'fa-spin': false }" />
            {{ library.ingesting ? "导入中…" : "选择文件" }}
          </button>
        </div>
      </section>

      <!-- 统计 -->
      <section class="grid grid-cols-2 gap-4 md:grid-cols-4">
        <div class="panel p-4">
          <div class="text-2xl font-semibold text-jade-400">{{ books.length }}</div>
          <div class="text-xs text-slate-500">藏书数量</div>
        </div>
        <div class="panel p-4">
          <div class="text-2xl font-semibold text-gold-400">{{ library.totalChapters }}</div>
          <div class="text-xs text-slate-500">总章节数</div>
        </div>
        <div class="panel p-4">
          <div class="text-2xl font-semibold text-jade-400">{{ totalExtracted }}</div>
          <div class="text-xs text-slate-500">已抽取章节</div>
        </div>
        <div class="panel p-4">
          <div class="text-2xl font-semibold text-gold-400">
            {{ library.activeBook?.format ?? "—" }}
          </div>
          <div class="text-xs text-slate-500">当前书籍格式</div>
        </div>
      </section>

      <!-- 抽取运行器 -->
      <ExtractionRunner v-if="library.activeBookId" />

      <!-- 书库列表 -->
      <section>
        <h3 class="mb-3 text-sm font-semibold uppercase tracking-widest text-slate-400">
          <i class="fa-solid fa-books mr-2 text-jade-500" />全部藏书
        </h3>
        <p v-if="!books.length" class="panel p-8 text-center text-sm text-slate-500">暂无藏书。</p>
        <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          <article
            v-for="book in books"
            :key="book.id"
            class="panel flex flex-col p-4"
            :class="{ 'ring-1 ring-jade-500/60': book.id === library.activeBookId }"
          >
            <div class="flex items-start gap-3">
              <div class="grid h-10 w-10 shrink-0 place-items-center rounded-lg bg-ink-600 text-jade-400">
                <i class="fa-solid fa-book" />
              </div>
              <div class="min-w-0 flex-1">
                <h4 class="truncate font-medium text-slate-100">{{ book.title }}</h4>
                <p class="text-xs text-slate-500">{{ book.author || "佚名" }} · {{ book.format }}</p>
              </div>
            </div>
            <div class="mt-3 space-y-1">
              <div class="flex justify-between text-[11px] text-slate-500">
                <span>抽取进度</span>
                <span>{{ book.extracted_chapters }}/{{ book.total_chapters }}</span>
              </div>
              <div class="h-1.5 overflow-hidden rounded-full bg-ink-600">
                <div
                  class="h-full rounded-full bg-jade-500 transition-all"
                  :style="{ width: (book.total_chapters ? (book.extracted_chapters / book.total_chapters) * 100 : 0) + '%' }"
                />
              </div>
            </div>
            <div class="mt-4 flex flex-wrap gap-2">
              <button class="btn-ghost !py-1.5" @click="library.open(book.id); ui.setView('canvas')">
                <i class="fa-solid fa-diagram-project" />画布
              </button>
              <button class="btn-primary !py-1.5" @click="library.open(book.id); ui.setView('library')">
                <i class="fa-solid fa-wand-magic-sparkles" />抽取
              </button>
              <button class="btn-ghost !py-1.5" @click="exportHtml(book.id)">
                <i class="fa-solid fa-file-export" />导出
              </button>
              <button class="btn-danger ml-auto !py-1.5" @click="library.remove(book.id)">
                <i class="fa-solid fa-trash" />
              </button>
            </div>
          </article>
        </div>
      </section>
    </div>
  </div>
</template>
