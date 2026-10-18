<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useLibraryStore } from "@/stores/library";
import BookCandidateNode from "@/components/BookCandidateNode.vue";
import type { BookCandidate } from "@/types/schema";

const library = useLibraryStore();

const pending = computed(() => library.pending);
const selectedIds = ref<string[]>([]);
const expandedIds = ref<string[]>([]);

watch(
  pending,
  (p) => {
    const top = p?.inspection.candidates ?? [];
    selectedIds.value = top.map((c) => c.id);
    expandedIds.value = top.map((c) => c.id);
  },
  { immediate: true },
);

const candidates = computed(() => pending.value?.inspection.candidates ?? []);

function flatten(nodes: BookCandidate[]): BookCandidate[] {
  return nodes.flatMap((node) => [node, ...flatten(node.children)]);
}

const allNodes = computed(() => flatten(candidates.value));
const nodeById = computed(() => new Map(allNodes.value.map((node) => [node.id, node])));
const selectedNodes = computed(() =>
  allNodes.value.filter((node) => selectedIds.value.includes(node.id)),
);
const selectedChapters = computed(() =>
  selectedNodes.value.reduce((sum, node) => sum + node.chapter_count, 0),
);

function countDescendants(node: BookCandidate): number {
  return node.children.reduce((sum, child) => sum + 1 + countDescendants(child), 0);
}

const selectedDescendants = computed(() =>
  selectedNodes.value.reduce((sum, node) => sum + countDescendants(node), 0),
);

const depthOptions = computed(() => {
  const groups = new Map<number, BookCandidate[]>();
  for (const node of allNodes.value) {
    const list = groups.get(node.depth) ?? [];
    list.push(node);
    groups.set(node.depth, list);
  }
  return [...groups.entries()]
    .sort((a, b) => a[0] - b[0])
    .map(([depth, nodes]) => ({ depth, nodes }));
});

function depthLabel(depth: number): string {
  return depth === 0 ? "顶层" : `第 ${depth + 1} 层`;
}

function selectDepth(depth: number) {
  selectedIds.value = allNodes.value
    .filter((node) => node.depth === depth)
    .map((node) => node.id);
}

function descendantIds(node: BookCandidate): string[] {
  return node.children.flatMap((child) => [child.id, ...descendantIds(child)]);
}

function toggle(id: string, checked: boolean) {
  const node = nodeById.value.get(id);
  if (!node) return;
  const removed = new Set([id, ...descendantIds(node)]);
  if (checked) {
    selectedIds.value = [...selectedIds.value.filter((x) => !removed.has(x)), id];
  } else {
    selectedIds.value = selectedIds.value.filter((x) => !removed.has(x));
  }
}

function toggleExpand(id: string) {
  expandedIds.value = expandedIds.value.includes(id)
    ? expandedIds.value.filter((x) => x !== id)
    : [...expandedIds.value, id];
}

function clearAll() {
  selectedIds.value = [];
}

function confirmSelected() {
  const ids = selectedNodes.value.map((node) => node.id);
  if (!ids.length) return;
  library.confirmPending(true, ids);
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
            <i class="fa-solid fa-sitemap" />
          </div>
          <div>
            <h3 class="font-semibold text-slate-100">选择要导入的目录节点</h3>
            <p class="text-xs text-slate-400">
              《{{ pending.inspection.title }}》包含多层级目录；勾选任意节点作为一本书，其下级会作为该书的章节。
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

          <div class="mt-3 flex flex-wrap items-center justify-between gap-2">
            <p class="text-xs text-slate-400">
              已选 <span class="font-medium tabular-nums text-slate-200">{{ selectedNodes.length }}</span> 本 ·
              <span class="tabular-nums">{{ selectedChapters }}</span> 章
              <template v-if="selectedDescendants"> · {{ selectedDescendants }} 个下级节点</template>
            </p>
            <div class="flex flex-wrap items-center gap-1.5">
              <span class="text-[11px] text-slate-500">按层级：</span>
              <button
                v-for="option in depthOptions"
                :key="option.depth"
                class="rounded-md border border-ink-500/60 px-1.5 py-0.5 text-[11px] text-slate-400 transition hover:border-jade-500/60 hover:text-jade-300"
                @click="selectDepth(option.depth)"
              >
                {{ depthLabel(option.depth) }}（{{ option.nodes.length }}）
              </button>
              <button class="btn-ghost !px-2 !py-1 text-xs" @click="clearAll">清空</button>
            </div>
          </div>

          <div class="mt-2 overflow-hidden rounded-lg border border-ink-600/60 bg-ink-900/25 p-1.5">
            <BookCandidateNode
              v-for="node in candidates"
              :key="node.id"
              :node="node"
              :selected-ids="selectedIds"
              :expanded-ids="expandedIds"
              @toggle="toggle"
              @toggle-expand="toggleExpand"
            />
          </div>

          <p class="mt-3 text-[11px] leading-relaxed text-slate-500">
            勾选合集会把整个合集作为一本书；若要分别导入合集内的书，请展开后勾选子节点。
            未勾选的节点不会导入。
          </p>
        </div>

        <div class="flex flex-wrap justify-end gap-2 border-t border-ink-600/60 px-5 py-4">
          <button class="btn-ghost" @click="library.cancelPending()">取消</button>
          <button class="btn-ghost" @click="library.confirmPending(false)">
            <i class="fa-solid fa-book" />作为单本导入
          </button>
          <button class="btn-primary" :disabled="!selectedNodes.length" @click="confirmSelected">
            <i class="fa-solid fa-layer-group" />导入选中（{{ selectedNodes.length }} 本）
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
