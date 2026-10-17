<script setup lang="ts">
import { computed, ref, watch } from "vue";
import SpaceGraph from "@/components/SpaceGraph.vue";
import TimeScrubber from "@/components/TimeScrubber.vue";
import { useLibraryStore } from "@/stores/library";
import { cleanPlace, cleanWorld } from "@/utils/space";
import type { SiteSelection } from "@/types/schema";

const library = useLibraryStore();
const activeIndex = ref(0);
const selectedEntityId = ref<number | null>(null);
const selectedSite = ref<SiteSelection | null>(null);

const timeline = computed(() => library.timeline);
const chapters = computed(() => timeline.value?.chapters ?? []);
const entities = computed(() => timeline.value?.entities ?? []);
const events = computed(() => timeline.value?.events ?? []);

const activeChapterIndex = computed(() => chapters.value[activeIndex.value]?.chapter_index ?? -1);
const selectedEntity = computed(() => entities.value.find((e) => e.id === selectedEntityId.value) ?? null);
const selectedEvents = computed(() =>
  selectedEntityId.value == null ? [] : events.value.filter((e) => e.entity_id === selectedEntityId.value),
);

const worldOf = (w?: string | null) => cleanWorld(w);
const siteOf = (c: { world?: string | null; region?: string | null; site?: string | null }) =>
  cleanPlace(c.site) || cleanPlace(c.region) || worldOf(c.world) || "未标注空间";

/// 选中地点的元数据：章节、人物、物品、时间标记。
const siteMeta = computed(() => {
  if (!selectedSite.value) return null;
  const { world, site } = selectedSite.value;
  const indexes = new Set<number>();
  const siteChapters = chapters.value.filter((c) => {
    if (worldOf(c.world) === world && siteOf(c) === site) {
      indexes.add(c.chapter_index);
      return true;
    }
    return false;
  });
  const evs = events.value.filter((e) => indexes.has(e.chapter_index));
  const chars = entities.value.filter(
    (e) => e.entity_type === "CHARACTER" && evs.some((v) => v.entity_id === e.id),
  );
  const items = entities.value.filter(
    (e) =>
      e.entity_type !== "CHARACTER" &&
      (evs.some((v) => v.entity_id === e.id) || evs.some((v) => v.acquired_site?.includes(site))),
  );
  const temporals = siteChapters
    .filter((c) => c.temporal_type && c.temporal_type !== "CONTINUOUS" && c.temporal_type !== "UNCERTAIN")
    .map((c) => ({ index: c.chapter_index, type: c.temporal_type as string, days: c.time_offset_days ?? 0 }));
  return {
    world: cleanWorld(world),
    site,
    region: cleanPlace(siteChapters[0]?.region) || null,
    chapters: siteChapters,
    chars,
    items,
    temporals,
  };
});

watch(chapters, () => {
  activeIndex.value = 0;
  selectedEntityId.value = null;
  selectedSite.value = null;
});

/// 接受章节序号（chapter_index），转换为数组下标。
function seek(chapterIndex: number) {
  const pos = chapters.value.findIndex((c) => c.chapter_index === chapterIndex);
  const next = pos >= 0 ? pos : chapterIndex;
  activeIndex.value = Math.max(0, Math.min(chapters.value.length - 1, next));
}
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
          请先在「图书馆」导入电子书并运行批量抽取，抽取完成后即可在此查看“时间 × 空间”矩阵：每行一个地点，横轴为章节，轨迹为人物行踪。
        </p>
      </div>
    </div>

    <div v-else class="flex h-full min-h-0 flex-col">
      <div class="flex min-w-0 flex-1 flex-col">
          <div class="relative min-h-0 flex-1">
            <SpaceGraph
              :chapters="chapters"
              :entities="entities"
              :events="events"
              :active-index="activeIndex"
              :highlight-entity-id="selectedEntityId"
              @select="selectedSite = $event"
              @select-entity="selectedEntityId = $event"
              @update:active-index="activeIndex = $event"
            />

            <!-- 地点元数据面板 -->
            <transition name="fade">
              <div
                v-if="siteMeta"
                class="absolute right-3 top-3 w-72 overflow-hidden rounded-xl border border-ink-500/60 bg-ink-800/90 shadow-xl backdrop-blur"
              >
                <div class="flex items-start gap-2 border-b border-ink-600/60 px-4 py-3">
                  <i class="fa-solid fa-location-dot mt-0.5 text-jade-400" />
                  <div class="min-w-0 flex-1">
                    <h4 class="truncate font-semibold text-slate-100">{{ siteMeta.site }}</h4>
                    <p v-if="siteMeta.world || siteMeta.region" class="text-[11px] text-slate-500">
                      <template v-if="siteMeta.world">{{ siteMeta.world }}</template>
                      <template v-if="siteMeta.world && siteMeta.region"> · </template>
                      <template v-if="siteMeta.region">{{ siteMeta.region }}</template>
                    </p>
                  </div>
                  <button class="text-slate-500 hover:text-slate-200" @click="selectedSite = null">
                    <i class="fa-solid fa-xmark" />
                  </button>
                </div>

                <div class="max-h-[22rem] space-y-3 overflow-y-auto px-4 py-3 text-xs">
                  <div>
                    <div class="mb-1 text-[11px] uppercase tracking-wide text-slate-500">出现章节 ({{ siteMeta.chapters.length }})</div>
                    <div class="flex flex-wrap gap-1">
                      <button
                        v-for="c in siteMeta.chapters"
                        :key="c.id"
                        class="rounded border px-1.5 py-0.5 text-[11px] transition"
                        :class="
                          c.chapter_index === activeChapterIndex
                            ? 'border-gold-400 text-gold-400'
                            : 'border-ink-600 text-slate-400 hover:border-jade-500 hover:text-jade-400'
                        "
                        @click="seek(c.chapter_index)"
                      >
                        {{ c.chapter_index + 1 }}
                      </button>
                    </div>
                  </div>

                  <div v-if="siteMeta.chars.length">
                    <div class="mb-1 text-[11px] uppercase tracking-wide text-slate-500">人物 ({{ siteMeta.chars.length }})</div>
                    <div class="flex flex-wrap gap-1">
                      <button
                        v-for="p in siteMeta.chars"
                        :key="p.id"
                        class="rounded-full border border-jade-500/40 bg-jade-500/10 px-2 py-0.5 text-[11px] text-jade-300 transition hover:border-jade-400 hover:bg-jade-500/20"
                        :title="p.description || '点击查看人物谱系'"
                        @click="selectedEntityId = p.id"
                      >
                        {{ p.name }}
                      </button>
                    </div>
                  </div>

                  <div v-if="siteMeta.items.length">
                    <div class="mb-1 text-[11px] uppercase tracking-wide text-slate-500">物品 / 功法 ({{ siteMeta.items.length }})</div>
                    <div class="flex flex-wrap gap-1">
                      <button
                        v-for="it in siteMeta.items"
                        :key="it.id"
                        class="rounded-full border border-gold-400/40 bg-gold-400/10 px-2 py-0.5 text-[11px] text-gold-400 hover:border-gold-400"
                        :title="it.description || ''"
                        @click="selectedEntityId = it.id"
                      >
                        {{ it.name }}
                      </button>
                    </div>
                  </div>

                  <div v-if="siteMeta.temporals.length">
                    <div class="mb-1 text-[11px] uppercase tracking-wide text-slate-500">时间标记</div>
                    <ul class="space-y-1 text-slate-400">
                      <li v-for="t in siteMeta.temporals" :key="t.index">
                        <span class="text-slate-500">第 {{ t.index + 1 }} 章</span>
                        <span class="ml-1 text-jade-400">{{ t.type }}</span>
                        <span v-if="t.days" class="ml-1 text-gold-400">+{{ t.days }} 天</span>
                      </li>
                    </ul>
                  </div>

                  <p v-if="!siteMeta.chars.length && !siteMeta.items.length && !siteMeta.temporals.length" class="text-slate-500">
                    暂无更多元数据。
                  </p>
                </div>
              </div>
            </transition>
          </div>

          <div class="p-3">
            <TimeScrubber v-model="activeIndex" :chapters="chapters" />
          </div>
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
              @click="seek(ev.chapter_index)"
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
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
