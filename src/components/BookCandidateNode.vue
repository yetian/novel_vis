<script setup lang="ts">
import { computed } from "vue";
import type { BookCandidate } from "@/types/schema";

const props = defineProps<{
  node: BookCandidate;
  selectedIds: string[];
  expandedIds: string[];
  ancestorSelected?: boolean;
}>();

const emit = defineEmits<{
  toggle: [id: string, checked: boolean];
  toggleExpand: [id: string];
}>();

const selfSelected = computed(() => props.selectedIds.includes(props.node.id));
const covered = computed(() => !!props.ancestorSelected || selfSelected.value);
const expanded = computed(() => props.expandedIds.includes(props.node.id));
const hasChildren = computed(() => props.node.children.length > 0);
const indent = computed(() => 8 + props.node.depth * 14);
</script>

<template>
  <div>
    <div
      class="flex items-center gap-2 rounded-md py-2 pr-2 transition"
      :class="
        selfSelected
          ? 'bg-jade-500/[0.08]'
          : covered
            ? 'bg-ink-700/30'
            : 'hover:bg-ink-700/40'
      "
      :style="{ paddingLeft: indent + 'px' }"
    >
      <button
        v-if="hasChildren"
        class="grid h-5 w-5 shrink-0 place-items-center rounded text-slate-500 transition hover:bg-ink-600 hover:text-slate-200"
        :title="expanded ? '收起' : '展开'"
        type="button"
        @click="emit('toggleExpand', node.id)"
      >
        <i class="fa-solid fa-chevron-right text-[10px] transition-transform" :class="{ 'rotate-90': expanded }" />
      </button>
      <span v-else class="h-5 w-5 shrink-0" />

      <label class="flex min-w-0 flex-1 cursor-pointer items-center gap-2">
        <input
          type="checkbox"
          class="h-3.5 w-3.5 shrink-0 accent-jade-500"
          :checked="covered"
          :disabled="ancestorSelected"
          @change="emit('toggle', node.id, ($event.target as HTMLInputElement).checked)"
        />

        <span class="min-w-0 flex-1">
          <span class="flex items-center gap-2">
            <span
              class="truncate text-sm"
              :class="covered ? 'font-medium text-slate-100' : 'text-slate-300'"
              :title="node.title"
            >
              {{ node.title }}
            </span>
            <span
              v-if="selfSelected"
              class="shrink-0 rounded-full border border-jade-500/40 bg-jade-500/10 px-1.5 py-0.5 text-[9px] text-jade-300"
            >
              作为一本书
            </span>
          </span>
          <span class="mt-0.5 block text-[11px] text-slate-500">
            第 {{ node.start_index + 1 }}–{{ node.end_index + 1 }} 章 · {{ node.chapter_count }} 章
            <template v-if="hasChildren"> · {{ node.children.length }} 个下级</template>
          </span>
        </span>
      </label>
    </div>

    <div v-if="hasChildren && expanded">
      <BookCandidateNode
        v-for="child in node.children"
        :key="child.id"
        :node="child"
        :selected-ids="selectedIds"
        :expanded-ids="expandedIds"
        :ancestor-selected="covered"
        @toggle="(id, checked) => emit('toggle', id, checked)"
        @toggle-expand="(id) => emit('toggleExpand', id)"
      />
    </div>
  </div>
</template>
