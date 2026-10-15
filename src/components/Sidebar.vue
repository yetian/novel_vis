<script setup lang="ts">
import { computed } from "vue";
import { useLibraryStore } from "@/stores/library";
import { useUiStore } from "@/stores/ui";

const library = useLibraryStore();
const ui = useUiStore();

const formatIcon = (format: string) =>
  format === "EPUB" ? "fa-book" : format === "AZW3" ? "fa-book-bookmark" : "fa-book-open";

const progressOf = (id: number) => {
  const book = library.books.find((b) => b.id === id);
  if (!book || !book.total_chapters) return 0;
  return Math.round((book.extracted_chapters / book.total_chapters) * 100);
};

const hasBooks = computed(() => library.books.length > 0);
</script>

<template>
  <aside class="flex h-full w-72 shrink-0 flex-col border-r border-ink-600/60 bg-ink-800/40">
    <div class="flex items-center justify-between px-4 py-3">
      <span class="text-xs font-semibold uppercase tracking-widest text-slate-400">
        <i class="fa-solid fa-layer-group mr-2 text-jade-500" />书库
      </span>
      <button class="btn-ghost !px-2 !py-1" title="刷新" @click="library.refresh()">
        <i class="fa-solid fa-rotate" :class="{ 'animate-spin': library.loading }" />
      </button>
    </div>

    <div class="flex-1 space-y-2 overflow-y-auto px-3 pb-3">
      <p v-if="!hasBooks" class="px-2 py-8 text-center text-sm text-slate-500">
        <i class="fa-regular fa-folder-open mb-2 block text-2xl" />
        尚无书籍，请拖入 EPUB / MOBI / AZW3
      </p>

      <button
        v-for="book in library.books"
        :key="book.id"
        class="w-full rounded-lg border px-3 py-2.5 text-left transition"
        :class="
          book.id === library.activeBookId
            ? 'border-jade-500 bg-jade-500/10'
            : 'border-ink-600/60 hover:border-ink-500 hover:bg-ink-700/50'
        "
        @click="library.open(book.id); ui.setView('canvas')"
      >
        <div class="flex items-center gap-2">
          <i class="fa-solid text-jade-400" :class="formatIcon(book.format)" />
          <span class="flex-1 truncate text-sm font-medium text-slate-100">{{ book.title }}</span>
          <span class="rounded bg-ink-600 px-1.5 py-0.5 text-[10px] text-slate-400">{{ book.format }}</span>
        </div>
        <div class="mt-1 flex items-center justify-between text-[11px] text-slate-500">
          <span>{{ book.author || "佚名" }}</span>
          <span>{{ book.extracted_chapters }}/{{ book.total_chapters }} 章</span>
        </div>
        <div class="mt-1.5 h-1 overflow-hidden rounded-full bg-ink-600">
          <div class="h-full rounded-full bg-jade-500 transition-all" :style="{ width: progressOf(book.id) + '%' }" />
        </div>
      </button>
    </div>

    <div class="border-t border-ink-600/60 px-4 py-3 text-[11px] leading-relaxed text-slate-500">
      <i class="fa-solid fa-shield-halved mr-1 text-jade-500" />
      全部数据本地处理，原文仅驻留内存。
    </div>
  </aside>
</template>
