<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useLibraryStore } from "@/stores/library";

const library = useLibraryStore();

const pending = computed(() => library.pending);
const selected = ref<number[]>([]);

watch(
  pending,
  (p) => {
    selected.value = p ? p.inspection.candidates.map((_, i) => i) : [];
  },
  { immediate: true },
);

const candidates = computed(() => pending.value?.inspection.candidates ?? []);
const allSelected = computed(
  () => candidates.value.length > 0 && selected.value.length === candidates.value.length,
);
const selectedChapters = computed(() =>
  candidates.value.reduce(
    (sum, c, i) => sum + (selected.value.includes(i) ? c.chapter_count : 0),
    0,
  ),
);
const selectedSubchapters = computed(() =>
  candidates.value.reduce(
    (sum, c, i) => sum + (selected.value.includes(i) ? c.subchapters.length : 0),
    0,
  ),
);

function toggle(index: number) {
  selected.value = selected.value.includes(index)
    ? selected.value.filter((i) => i !== index)
    : [...selected.value, index].sort((a, b) => a - b);
}

function toggleAll() {
  selected.value = allSelected.value ? [] : candidates.value.map((_, i) => i);
}

function confirmSelected() {
  if (!selected.value.length) return;
  library.confirmPending(true, [...selected.value].sort((a, b) => a - b));
}
</script>

<template>
  <transition name="fade">
    <div
      v-if="pending"
      class="fixed inset-0 z-40 grid place-items-center bg-ink-900/70 p-4 backdrop-blur-sm"
      @click.self="library.cancelPending()"
    >
      <div class="panel flex max-h-[88vh] w-full max-w-2xl flex-col overflow-hidden">
        <div class="flex items-center gap-3 border-b border-ink-600/60 px-5 py-4">
          <div class="grid h-10 w-10 place-items-center rounded-lg bg-gold-400/15 text-gold-400">
            <i class="fa-solid fa-book-open-reader" />
          </div>
          <div>
            <h3 class="font-semibold text-slate-100">选择要导入的主章节</h3>
            <p class="text-xs text-slate-400">
              《{{ pending.inspection.title }}》似乎包含多本书；勾选主章节，其子章节会作为同一本书的章节导入。
            </p>
          </div>
        </div>

        <div class="min-h-0 flex-1 overflow-y-auto px-5 py-4">
          <div class="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-slate-400">
            <span>{{ pending.inspection.format }}</span>
            <span>共 {{ pending.inspection.total_chapters }} 章</span>
            <span v-for="s in pending.inspection.signals" :key="s" class="text-jade-400">
              <i class="fa-solid fa-circle-info mr-1" />{{ s }}
            </span>
          </div>

          <div class="mt-3 flex items-center justify-between gap-3">
            <p class="text-xs text-slate-400">
              已选 <span class="font-medium tabular-nums text-slate-200">{{ selected.length }}</span> 本 ·
              <span class="tabular-nums">{{ selectedChapters }}</span> 章
              <template v-if="selectedSubchapters"> · {{ selectedSubchapters }} 个子章节</template>
            </p>
            <button class="btn-ghost !px-2 !py-1 text-xs" @click="toggleAll">
              {{ allSelected ? "清空选择" : "全选" }}
            </button>
          </div>

          <div class="mt-2 space-y-2">
            <div
              v-for="(c, i) in candidates"
              :key="c.title + i"
              class="overflow-hidden rounded-lg border transition"
              :class="
                selected.includes(i)
                  ? 'border-jade-500/50 bg-jade-500/[0.06]'
                  : 'border-ink-600/60 bg-ink-900/20'
              "
            >
              <label class="flex cursor-pointer items-center gap-3 px-3 py-2.5">
                <input
                  type="checkbox"
                  class="h-3.5 w-3.5 shrink-0 accent-jade-500"
                  :checked="selected.includes(i)"
                  @change="toggle(i)"
                />
                <span class="grid h-6 w-6 shrink-0 place-items-center rounded bg-ink-600 text-[11px] text-slate-400">
                  {{ i + 1 }}
                </span>
                <span class="min-w-0 flex-1">
                  <span class="block truncate text-sm text-slate-200">{{ c.title }}</span>
                  <span class="mt-0.5 block text-[11px] text-slate-500">
                    第 {{ c.start_index + 1 }}–{{ c.end_index + 1 }} 章 · {{ c.chapter_count }} 章
                    <template v-if="c.subchapters.length"> · {{ c.subchapters.length }} 个子章节</template>
                  </span>
                </span>
              </label>

              <details v-if="c.subchapters.length" class="border-t border-ink-600/50">
                <summary class="cursor-pointer px-3 py-1.5 text-[11px] text-slate-400 hover:text-slate-300">
                  查看子章节
                </summary>
                <ul class="max-h-36 space-y-0.5 overflow-y-auto px-3 pb-2 text-[11px] text-slate-500">
                  <li
                    v-for="(sub, si) in c.subchapters"
                    :key="sub.title + si"
                    class="flex items-center justify-between gap-3"
                    :style="{ paddingLeft: (sub.depth - 1) * 12 + 'px' }"
                  >
                    <span class="truncate">{{ sub.title }}</span>
                    <span class="shrink-0 tabular-nums">第 {{ sub.start_index + 1 }}–{{ sub.end_index + 1 }} 章</span>
                  </li>
                </ul>
              </details>
            </div>
          </div>

          <p class="mt-3 text-[11px] text-slate-500">
            未勾选的主章节不会导入；选中主章节范围内的子章节会自动归入该本书。
          </p>
        </div>

        <div class="flex flex-wrap justify-end gap-2 border-t border-ink-600/60 px-5 py-4">
          <button class="btn-ghost" @click="library.cancelPending()">取消</button>
          <button class="btn-ghost" @click="library.confirmPending(false)">
            <i class="fa-solid fa-book" />作为单本导入
          </button>
          <button class="btn-primary" :disabled="!selected.length" @click="confirmSelected">
            <i class="fa-solid fa-layer-group" />导入选中（{{ selected.length }} 本）
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
