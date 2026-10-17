<script setup lang="ts">
import { computed } from "vue";
import { useLibraryStore } from "@/stores/library";
import { useExtractionStore } from "@/stores/extraction";

const library = useLibraryStore();
const extraction = useExtractionStore();

const isExtracting = (id: number) => extraction.runningBookIds.includes(id);

const formatIcon = (format: string) =>
  format === "EPUB" ? "fa-book" : format === "AZW3" ? "fa-book-bookmark" : "fa-book-open";

const progressOf = (id: number) => {
  const book = library.books.find((b) => b.id === id);
  if (!book || !book.total_chapters) return 0;
  return Math.round((book.extracted_chapters / book.total_chapters) * 100);
};

const isComplete = (id: number) => {
  const book = library.books.find((b) => b.id === id);
  return !!book && book.total_chapters > 0 && book.extracted_chapters >= book.total_chapters;
};

const hasBooks = computed(() => library.books.length > 0);
</script>

<template>
  <aside class="flex h-full w-72 shrink-0 flex-col border-r border-ink-500/40 bg-ink-800/60">
    <div class="flex items-start justify-between px-4 pb-3 pt-4">
      <div class="min-w-0">
        <div class="flex items-center gap-2">
          <span class="text-[13px] font-semibold tracking-wide text-slate-200">书库</span>
          <span class="rounded-full bg-ink-600/80 px-2 py-0.5 text-[11px] font-medium tabular-nums text-slate-300">
            {{ library.books.length }}
          </span>
        </div>
        <p class="mt-1 text-xs text-slate-400">点击书籍进入总览</p>
      </div>
      <button
        class="btn-ghost !rounded-lg !p-2 text-slate-300"
        title="刷新书库"
        aria-label="刷新书库"
        @click="library.refresh()"
      >
        <i class="fa-solid fa-rotate text-xs" :class="{ 'animate-spin': library.loading }" />
      </button>
    </div>

    <div class="flex-1 space-y-2 overflow-y-auto px-3 pb-4">
      <div v-if="!hasBooks" class="mx-1 mt-2 rounded-xl border border-dashed border-ink-500/60 px-4 py-10 text-center">
        <i class="fa-regular fa-folder-open mb-3 block text-2xl text-slate-500" />
        <p class="text-sm font-medium text-slate-300">书库还是空的</p>
        <p class="mt-1 text-xs leading-relaxed text-slate-400">拖入 EPUB / MOBI / AZW3，<br />或从主面板导入。</p>
      </div>

      <button
        v-for="book in library.books"
        :key="book.id"
        class="group relative w-full overflow-hidden rounded-xl border px-3.5 py-3 text-left transition duration-200"
        :class="
          book.id === library.activeBookId
            ? 'border-jade-500/70 bg-jade-500/[0.08] shadow-glow'
            : 'border-ink-500/40 bg-ink-700/35 hover:-translate-y-px hover:border-ink-500/80 hover:bg-ink-700/60'
        "
        :aria-current="book.id === library.activeBookId ? 'page' : undefined"
        @click="library.open(book.id, 'summary')"
      >
        <span
          v-if="book.id === library.activeBookId"
          class="absolute inset-y-3 left-0 w-0.5 rounded-r-full bg-jade-400"
        />

        <div class="flex items-start gap-3">
          <div
            class="grid h-10 w-10 shrink-0 place-items-center rounded-xl border border-ink-500/50 bg-gradient-to-br from-ink-600 to-ink-800 text-jade-400 shadow-sm"
          >
            <i class="fa-solid text-sm" :class="formatIcon(book.format)" />
          </div>

          <div class="min-w-0 flex-1">
            <div class="flex items-center gap-2">
              <span
                class="min-w-0 flex-1 truncate font-serif text-[15px] font-medium leading-snug text-slate-100"
                :title="book.title"
              >
                {{ book.title }}
              </span>
              <i
                v-if="isExtracting(book.id)"
                class="fa-solid fa-circle-notch fa-spin shrink-0 text-xs text-gold-400"
                title="抽取中"
              />
            </div>

            <div class="mt-1 flex min-w-0 items-center gap-1.5 text-xs text-slate-400">
              <span class="truncate">{{ book.author || "佚名" }}</span>
              <span class="text-slate-600">·</span>
              <span class="shrink-0 rounded bg-ink-600/80 px-1.5 py-0.5 text-[10px] font-medium tracking-wide text-slate-300">
                {{ book.format }}
              </span>
            </div>
          </div>
        </div>

        <div class="mt-3 flex items-center gap-2.5">
          <div class="h-1.5 flex-1 overflow-hidden rounded-full bg-ink-600">
            <div
              class="h-full rounded-full bg-gradient-to-r from-jade-600 to-jade-400 transition-all duration-500"
              :style="{ width: progressOf(book.id) + '%' }"
            />
          </div>
          <span class="w-9 text-right text-[11px] font-medium tabular-nums text-slate-400">
            {{ progressOf(book.id) }}%
          </span>
        </div>

        <div class="mt-1.5 flex items-center justify-between text-[11px]">
          <span class="tabular-nums text-slate-500">
            {{ book.extracted_chapters }} / {{ book.total_chapters }} 章
          </span>
          <span v-if="isExtracting(book.id)" class="font-medium text-gold-400">抽取中</span>
          <span v-else-if="isComplete(book.id)" class="font-medium text-jade-400">已完成</span>
          <span v-else-if="book.extracted_chapters > 0" class="text-slate-400">继续抽取</span>
          <span v-else class="text-slate-500">待抽取</span>
        </div>
      </button>
    </div>

    <div class="border-t border-ink-500/40 px-4 py-3 text-xs leading-relaxed text-slate-400">
      <i class="fa-solid fa-shield-halved mr-1.5 text-jade-500" />
      全部数据本地处理，原文仅驻留内存。
    </div>
  </aside>
</template>
