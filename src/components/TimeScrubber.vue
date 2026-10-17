<script setup lang="ts">
import { computed } from "vue";
import { cleanPlace, cleanWorld } from "@/utils/space";
import type { ChapterRow } from "@/types/schema";

const props = defineProps<{
  chapters: ChapterRow[];
  modelValue: number;
}>();
const emit = defineEmits<{ (e: "update:modelValue", value: number): void }>();

// 虚拟故事时间线：按时间偏移累加，形成天数轴。
const cumulativeDays = computed(() => {
  let acc = 0;
  return props.chapters.map((c) => {
    acc += c.time_offset_days ?? 0;
    return acc;
  });
});

const maxDays = computed(() => cumulativeDays.value[cumulativeDays.value.length - 1] ?? 0);

const virtualDays = computed(() => cumulativeDays.value[props.modelValue] ?? 0);

const currentChapter = computed(() => props.chapters[props.modelValue]);
const currentPlace = computed(() => {
  const c = currentChapter.value;
  if (!c) return "";
  return cleanPlace(c.site) || cleanPlace(c.region) || cleanWorld(c.world);
});

function onChapterInput(e: Event) {
  emit("update:modelValue", Number((e.target as HTMLInputElement).value));
}

function onTimeInput(e: Event) {
  const target = Number((e.target as HTMLInputElement).value);
  // 找到与目标虚拟时间最接近的章节
  let best = 0;
  let bestDist = Infinity;
  cumulativeDays.value.forEach((d, i) => {
    const dist = Math.abs(d - target);
    if (dist < bestDist) {
      bestDist = dist;
      best = i;
    }
  });
  emit("update:modelValue", best);
}
</script>

<template>
  <div class="panel px-4 py-3">
    <div class="flex items-center justify-between text-xs text-slate-400">
      <span class="flex items-center gap-2">
        <i class="fa-solid fa-book text-jade-500" />
        章节轴
        <span class="text-slate-200">{{ modelValue + 1 }} / {{ chapters.length }}</span>
      </span>
      <span class="flex items-center gap-2">
        <i class="fa-solid fa-hourglass-half text-gold-400" />
        虚拟时间线
        <span class="text-slate-200">第 {{ virtualDays }} 天</span>
        <span class="text-slate-600">/ {{ maxDays }} 天</span>
      </span>
    </div>

    <div class="mt-2 space-y-2">
      <input
        type="range"
        class="w-full accent-jade-500"
        min="0"
        :max="Math.max(0, chapters.length - 1)"
        :value="modelValue"
        @input="onChapterInput"
      />
      <input
        type="range"
        class="w-full accent-gold-400"
        min="0"
        :max="Math.max(0, maxDays)"
        :value="virtualDays"
        @input="onTimeInput"
      />
    </div>

    <div v-if="currentChapter" class="mt-1 flex flex-wrap items-center gap-2 text-[11px]">
      <span class="text-gold-400">第 {{ currentChapter.chapter_index + 1 }} 章 · {{ currentChapter.title }}</span>
      <span v-if="currentPlace" class="rounded-full border border-gold-400/40 px-2 py-0.5 text-gold-400">
        <i class="fa-solid fa-location-dot mr-1" />{{ currentPlace }}
        <template v-if="cleanWorld(currentChapter.world)"> · {{ cleanWorld(currentChapter.world) }}</template>
      </span>
      <span
        v-if="currentChapter.temporal_type"
        class="rounded-full border border-jade-500/40 px-2 py-0.5 text-jade-400"
      >
        {{ currentChapter.temporal_type }}
      </span>
      <span
        v-if="currentChapter.movement_type && currentChapter.movement_type !== 'STAY'"
        class="rounded-full border border-ink-500 px-2 py-0.5 text-slate-400"
      >
        {{ currentChapter.movement_type }}
      </span>
    </div>
  </div>
</template>
