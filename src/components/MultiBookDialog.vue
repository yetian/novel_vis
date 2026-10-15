<script setup lang="ts">
import { computed } from "vue";
import { useLibraryStore } from "@/stores/library";

const library = useLibraryStore();

const pending = computed(() => library.pending);

const totalCandidateChapters = computed(
  () => pending.value?.inspection.candidates.reduce((sum, c) => sum + c.chapter_count, 0) ?? 0,
);
</script>

<template>
  <transition name="fade">
    <div
      v-if="pending"
      class="fixed inset-0 z-40 grid place-items-center bg-ink-900/70 p-4 backdrop-blur-sm"
      @click.self="library.cancelPending()"
    >
      <div class="panel w-full max-w-lg overflow-hidden">
        <div class="flex items-center gap-3 border-b border-ink-600/60 px-5 py-4">
          <div class="grid h-10 w-10 place-items-center rounded-lg bg-gold-400/15 text-gold-400">
            <i class="fa-solid fa-book-open-reader" />
          </div>
          <div>
            <h3 class="font-semibold text-slate-100">检测到可能为合订本</h3>
            <p class="text-xs text-slate-500">该文件似乎包含多本书，请确认导入方式。</p>
          </div>
        </div>

        <div class="px-5 py-4">
          <p class="text-sm text-slate-300">
            《{{ pending.inspection.title }}》
            <span class="text-slate-500">
              · {{ pending.inspection.format }} · 共 {{ pending.inspection.total_chapters }} 章
            </span>
          </p>
          <p v-for="s in pending.inspection.signals" :key="s" class="mt-1 text-xs text-jade-400">
            <i class="fa-solid fa-circle-info mr-1" />{{ s }}
          </p>

          <div class="mt-3 max-h-56 space-y-2 overflow-y-auto">
            <div
              v-for="(c, i) in pending.inspection.candidates"
              :key="c.title + i"
              class="flex items-center gap-3 rounded-lg border border-ink-600/60 px-3 py-2"
            >
              <span class="grid h-6 w-6 shrink-0 place-items-center rounded bg-ink-600 text-[11px] text-slate-400">
                {{ i + 1 }}
              </span>
              <span class="flex-1 truncate text-sm text-slate-200">{{ c.title }}</span>
              <span class="text-[11px] text-slate-500">
                第 {{ c.start_index + 1 }}–{{ c.end_index + 1 }} 章 · {{ c.chapter_count }} 章
              </span>
            </div>
          </div>

          <p class="mt-3 text-[11px] text-slate-500">
            候选覆盖 {{ totalCandidateChapters }} 章；未被覆盖的章节将归入《{{ pending.inspection.title }} · 其他》。
          </p>
        </div>

        <div class="flex flex-wrap justify-end gap-2 border-t border-ink-600/60 px-5 py-4">
          <button class="btn-ghost" @click="library.cancelPending()">取消</button>
          <button class="btn-ghost" @click="library.confirmPending(false)">
            <i class="fa-solid fa-book" />作为单本导入
          </button>
          <button class="btn-primary" @click="library.confirmPending(true)">
            <i class="fa-solid fa-layer-group" />作为多本导入
          </button>
        </div>
      </div>
    </div>
  </transition>
</template>

<style scoped>
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
