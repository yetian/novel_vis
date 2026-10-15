<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import type { ChapterRow, EntityEventRow, EntityRow } from "@/types/schema";

const props = defineProps<{
  chapters: ChapterRow[];
  entities: EntityRow[];
  events: EntityEventRow[];
  activeIndex: number;
  highlightEntityId: number | null;
}>();
const emit = defineEmits<{ (e: "update:activeIndex", value: number): void }>();

const STEP = 40;
const PAD = 28;
const scroll = ref<HTMLDivElement | null>(null);

const marks = computed(() =>
  props.chapters.map((c, i) => ({
    i,
    chapter: c,
    x: PAD + i * STEP + STEP / 2,
    temporal: c.temporal_type || "UNCERTAIN",
    movement: c.movement_type || "STAY",
    days: c.time_offset_days ?? 0,
  })),
);

const totalWidth = computed(() => Math.max(640, PAD * 2 + props.chapters.length * STEP));

const eventChapters = computed(() => {
  if (props.highlightEntityId == null) return new Set<number>();
  const fromEvents = props.events
    .filter((e) => e.entity_id === props.highlightEntityId)
    .map((e) => e.chapter_index);
  if (fromEvents.length) return new Set(fromEvents);
  const entity = props.entities.find((e) => e.id === props.highlightEntityId);
  return new Set(entity?.first_appeared_chapter_index != null ? [entity.first_appeared_chapter_index] : []);
});

const temporalStyle: Record<string, { color: string; glyph: string; dir: number }> = {
  JUMP_FORWARD: { color: "#f0b429", glyph: "▲", dir: -1 },
  FLASHBACK: { color: "#4dabf7", glyph: "▼", dir: 1 },
  CONTINUOUS: { color: "#3a465e", glyph: "", dir: 0 },
  UNCERTAIN: { color: "#2a3345", glyph: "", dir: 0 },
};

const movementColor: Record<string, string> = {
  STAY: "transparent",
  LOCAL_MOVE: "#20c997",
  TELEPORT: "#b197fc",
  ASCENSION: "#f0b429",
  DUNGEON_ENTER: "#f03e3e",
  DUNGEON_EXIT: "#4dabf7",
};

function seek(i: number) {
  const clamped = Math.max(0, Math.min(props.chapters.length - 1, i));
  emit("update:activeIndex", clamped);
}

function onClickTrack(e: MouseEvent) {
  const target = e.currentTarget as HTMLElement;
  const rect = target.getBoundingClientRect();
  const x = e.clientX - rect.left;
  const i = Math.round((x - PAD - STEP / 2) / STEP);
  seek(i);
}

watch(
  () => props.activeIndex,
  async (i) => {
    await nextTick();
    const el = scroll.value;
    if (!el) return;
    const x = PAD + i * STEP + STEP / 2;
    const left = el.scrollLeft;
    const right = left + el.clientWidth;
    if (x < left + 60) el.scrollTo({ left: x - 60, behavior: "smooth" });
    else if (x > right - 60) el.scrollTo({ left: x - el.clientWidth + 60, behavior: "smooth" });
  },
);
</script>

<template>
  <div class="panel px-3 py-2">
    <div class="mb-1 flex items-center gap-3 text-xs">
      <span class="font-medium text-slate-300"><i class="fa-solid fa-timeline mr-1 text-jade-500" />时间线</span>
      <span class="text-slate-500">点击刻度跳转 · ▲ 时间跳跃 · ▼ 倒叙</span>
      <div class="ml-auto flex items-center gap-1">
        <button class="btn-ghost !px-2 !py-1" @click="seek(activeIndex - 1)"><i class="fa-solid fa-chevron-left" /></button>
        <button class="btn-ghost !px-2 !py-1" @click="seek(activeIndex + 1)"><i class="fa-solid fa-chevron-right" /></button>
      </div>
    </div>

    <div ref="scroll" class="overflow-x-auto overflow-y-hidden">
      <div class="relative h-24" :style="{ width: totalWidth + 'px' }" @click="onClickTrack">
        <!-- 主轴 -->
        <div class="absolute left-0 right-0 top-1/2 h-px -translate-y-1/2 bg-ink-500" />

        <div
          v-for="m in marks"
          :key="m.i"
          class="absolute -translate-x-1/2"
          :style="{ left: m.x + 'px', top: '50%' }"
        >
          <!-- 时间跳跃/倒叙标记 -->
          <div
            v-if="m.temporal === 'JUMP_FORWARD' || m.temporal === 'FLASHBACK'"
            class="absolute left-1/2 -translate-x-1/2 text-[10px] font-bold"
            :style="{
              color: temporalStyle[m.temporal].color,
              top: temporalStyle[m.temporal].dir < 0 ? '-26px' : '10px',
            }"
            :title="`${m.temporal}${m.days ? ' · +' + m.days + ' 天' : ''}`"
          >
            {{ temporalStyle[m.temporal].glyph }}<span v-if="m.days" class="ml-0.5 text-[9px]">{{ m.days }}天</span>
          </div>

          <!-- 地点移动标记 -->
          <span
            v-if="m.movement !== 'STAY'"
            class="absolute left-1/2 -translate-x-1/2 rounded-full"
            style="width: 6px; height: 6px; top: -3px"
            :style="{ background: movementColor[m.movement] }"
            :title="m.movement"
          />

          <!-- 章节刻度 -->
          <span
            class="absolute left-1/2 top-1/2 block -translate-x-1/2 -translate-y-1/2 rounded-full transition"
            :class="[
              m.i === activeIndex ? 'h-3.5 w-3.5 bg-gold-400 ring-2 ring-gold-400/40' : 'h-2 w-2 bg-ink-400',
              eventChapters.has(m.chapter.chapter_index) ? 'ring-2 ring-jade-500' : '',
            ]"
            :style="m.i === activeIndex ? '' : { background: eventChapters.has(m.chapter.chapter_index) ? '#20c997' : undefined }"
          />

          <!-- 章节序号 -->
          <span
            class="absolute left-1/2 top-1/2 mt-3 -translate-x-1/2 whitespace-nowrap text-[9px]"
            :class="m.i === activeIndex ? 'text-gold-400' : 'text-slate-600'"
          >
            {{ m.chapter.chapter_index + 1 }}
          </span>
        </div>

        <!-- 当前光标 -->
        <div
          class="pointer-events-none absolute top-0 h-full w-px bg-gold-400/60"
          :style="{ left: PAD + activeIndex * STEP + STEP / 2 + 'px' }"
        />
      </div>
    </div>
  </div>
</template>
