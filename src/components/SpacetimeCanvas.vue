<script setup lang="ts">
import { computed, ref, watch } from "vue";
import SpaceGraph from "@/components/SpaceGraph.vue";
import TimeScrubber from "@/components/TimeScrubber.vue";
import EntityDrawer from "@/components/EntityDrawer.vue";
import { useLibraryStore } from "@/stores/library";

const library = useLibraryStore();
const activeIndex = ref(0);
const selectedEntityId = ref<number | null>(null);

const timeline = computed(() => library.timeline);
const chapters = computed(() => timeline.value?.chapters ?? []);
const entities = computed(() => timeline.value?.entities ?? []);
const events = computed(() => timeline.value?.events ?? []);

const currentChapter = computed(() => chapters.value[activeIndex.value]);

const selectedEntity = computed(() => entities.value.find((e) => e.id === selectedEntityId.value) ?? null);

const selectedEvents = computed(() =>
  selectedEntityId.value == null ? [] : events.value.filter((e) => e.entity_id === selectedEntityId.value),
);

watch(chapters, () => {
  activeIndex.value = 0;
  selectedEntityId.value = null;
});
</script>

<template>
  <div class="h-full">
    <div v-if="!timeline || !chapters.length" class="grid h-full place-items-center p-8">
      <div class="max-w-md text-center">
        <div class="mx-auto mb-4 grid h-16 w-16 place-items-center rounded-2xl bg-ink-700 text-2xl text-slate-500">
          <i class="fa-solid fa-diagram-project" />
        </div>
        <h2 class="text-lg font-semibold text-slate-200">尚无可视化数据</h2>
        <p class="mt-2 text-sm text-slate-500">
          请先在「图书馆」导入电子书并运行批量抽取，抽取完成后即可在此查看拓扑空间图与双轴时间线。
        </p>
      </div>
    </div>

    <div v-else class="flex h-full min-h-0 flex-col">
      <div class="flex min-h-0 flex-1">
        <div class="flex min-w-0 flex-1 flex-col">
          <div class="min-h-0 flex-1">
            <SpaceGraph
              :chapters="chapters"
              :entities="entities"
              :events="events"
              :active-index="activeIndex"
              :highlight-entity-id="selectedEntityId"
            />
          </div>
          <div class="space-y-2 p-3">
            <TimeScrubber v-model="activeIndex" :chapters="chapters" />
            <div v-if="currentChapter" class="panel px-4 py-3 text-sm">
              <div class="flex items-center gap-2">
                <i class="fa-solid fa-quote-left text-jade-500" />
                <span class="text-xs uppercase tracking-wide text-slate-500">剧情摘要</span>
              </div>
              <p class="mt-1 text-slate-300">{{ currentChapter.raw_text_summary || "（暂无摘要）" }}</p>
            </div>
          </div>
        </div>

        <EntityDrawer
          :entities="entities"
          :events="events"
          :selected-id="selectedEntityId"
          @select="selectedEntityId = $event"
        />
      </div>

      <!-- 实体谱系 -->
      <transition name="slide-up">
        <div v-if="selectedEntity" class="border-t border-ink-600/60 bg-ink-800/70 p-4">
          <div class="flex items-center gap-3">
            <i
              class="fa-solid"
              :class="selectedEntity.entity_type === 'CHARACTER' ? 'fa-user text-jade-400' : 'fa-gem text-gold-400'"
            />
            <h3 class="font-semibold text-slate-100">{{ selectedEntity.name }} · 谱系</h3>
            <span class="text-xs text-slate-500">共 {{ selectedEvents.length }} 次事件</span>
            <button class="btn-ghost ml-auto !py-1" @click="selectedEntityId = null">
              <i class="fa-solid fa-xmark" />关闭
            </button>
          </div>
          <div class="mt-3 flex gap-3 overflow-x-auto pb-1">
            <button
              v-for="ev in selectedEvents"
              :key="ev.id"
              class="min-w-[200px] shrink-0 rounded-lg border border-ink-600/60 px-3 py-2 text-left transition hover:border-jade-500"
              @click="activeIndex = chapters.findIndex((c) => c.chapter_index === ev.chapter_index)"
            >
              <div class="text-xs text-gold-400">第 {{ ev.chapter_index + 1 }} 章 · {{ ev.action_type }}</div>
              <div v-if="ev.role" class="text-[11px] text-slate-400">角色：{{ ev.role }}</div>
              <div v-if="ev.acquired_site" class="text-[11px] text-slate-400">
                <i class="fa-solid fa-location-dot mr-1" />{{ ev.acquired_site }}
              </div>
              <div v-if="ev.impact_detail" class="mt-1 line-clamp-2 text-[11px] text-slate-500">{{ ev.impact_detail }}</div>
            </button>
          </div>
        </div>
      </transition>
    </div>
  </div>
</template>

<style scoped>
.slide-up-enter-active,
.slide-up-leave-active {
  transition: all 0.25s ease;
}
.slide-up-enter-from,
.slide-up-leave-to {
  transform: translateY(100%);
  opacity: 0;
}
</style>
