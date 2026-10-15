<script setup lang="ts">
import { computed, ref } from "vue";
import type { EntityEventRow, EntityRow } from "@/types/schema";

const props = defineProps<{
  entities: EntityRow[];
  events: EntityEventRow[];
  selectedId: number | null;
}>();
const emit = defineEmits<{ (e: "select", id: number | null): void }>();

const tab = ref<"CHARACTER" | "ITEM">("CHARACTER");
const query = ref("");

const isItemCat = (t: string) => t !== "CHARACTER";

const filtered = computed(() =>
  props.entities
    .filter((e) => (tab.value === "CHARACTER" ? e.entity_type === "CHARACTER" : isItemCat(e.entity_type)))
    .filter((e) => (query.value ? e.name.includes(query.value) : true)),
);

const count = (predicate: (t: string) => boolean) => props.entities.filter((e) => predicate(e.entity_type)).length;

function eventsOf(id: number) {
  return props.events.filter((e) => e.entity_id === id);
}

function categoryBadge(t: string) {
  return t === "CHARACTER" ? "人物" : t === "SKILL" ? "功法" : t === "LEGACY" ? "传承" : "物品";
}
</script>

<template>
  <div class="flex h-full w-80 flex-col border-l border-ink-600/60 bg-ink-800/40">
    <div class="flex items-center gap-1 border-b border-ink-600/60 p-2">
      <button
        class="flex-1 rounded-md px-3 py-1.5 text-sm transition"
        :class="tab === 'CHARACTER' ? 'bg-jade-500/15 text-jade-400' : 'text-slate-400 hover:bg-ink-700'"
        @click="tab = 'CHARACTER'"
      >
        <i class="fa-solid fa-user mr-1" />人物 ({{ count((t) => t === "CHARACTER") }})
      </button>
      <button
        class="flex-1 rounded-md px-3 py-1.5 text-sm transition"
        :class="tab === 'ITEM' ? 'bg-gold-400/15 text-gold-400' : 'text-slate-400 hover:bg-ink-700'"
        @click="tab = 'ITEM'"
      >
        <i class="fa-solid fa-gem mr-1" />物品功法 ({{ count(isItemCat) }})
      </button>
    </div>

    <div class="p-2">
      <input v-model="query" class="input !py-1.5 text-xs" placeholder="搜索名称…" />
    </div>

    <div class="flex-1 space-y-1.5 overflow-y-auto px-2 pb-3">
      <p v-if="!filtered.length" class="py-8 text-center text-xs text-slate-500">暂无数据</p>
      <button
        v-for="e in filtered"
        :key="e.id"
        class="w-full rounded-lg border px-3 py-2 text-left transition"
        :class="
          e.id === selectedId
            ? 'border-jade-500 bg-jade-500/10'
            : 'border-ink-600/50 hover:border-ink-500 hover:bg-ink-700/40'
        "
        @click="emit('select', e.id === selectedId ? null : e.id)"
      >
        <div class="flex items-center gap-2">
          <i
            class="fa-solid text-xs"
            :class="
              e.entity_type === 'CHARACTER'
                ? 'fa-user text-jade-400'
                : e.entity_type === 'SKILL'
                  ? 'fa-hand-fist text-gold-400'
                  : 'fa-gem text-gold-400'
            "
          />
          <span class="flex-1 truncate text-sm font-medium text-slate-100">{{ e.name }}</span>
          <span class="rounded bg-ink-600 px-1.5 py-0.5 text-[10px] text-slate-400">{{ categoryBadge(e.entity_type) }}</span>
        </div>
        <div class="mt-1 flex items-center gap-2 text-[11px] text-slate-500">
          <span v-if="e.first_appeared_chapter_index != null">
            <i class="fa-solid fa-location-dot mr-1" />首现 第 {{ e.first_appeared_chapter_index + 1 }} 章
          </span>
          <span v-if="eventsOf(e.id).length"><i class="fa-solid fa-link mr-1" />{{ eventsOf(e.id).length }} 次事件</span>
        </div>
        <p v-if="e.description" class="mt-1 line-clamp-2 text-[11px] text-slate-500">{{ e.description }}</p>
      </button>
    </div>
  </div>
</template>
