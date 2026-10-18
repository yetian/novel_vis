<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { api, isTauri } from "@/api/tauri";
import { useLibraryStore } from "@/stores/library";
import { useLlmStore } from "@/stores/llm";
import { useUiStore } from "@/stores/ui";
import { errText } from "@/utils/error";
import type { BiographyProgress, BiographyRow, DetailTarget } from "@/types/schema";

const props = defineProps<{
  open: boolean;
  target: DetailTarget | null;
  bookId: number | null;
}>();
const emit = defineEmits<{ close: [] }>();

const library = useLibraryStore();
const llm = useLlmStore();
const ui = useUiStore();

const tab = ref<"biography" | "metadata" | "timeline">("biography");
const biography = ref<BiographyRow | null>(null);
const loading = ref(false);
const generating = ref(false);
const bioProgress = ref<BiographyProgress | null>(null);
let unlistenProgress: UnlistenFn | null = null;

onMounted(async () => {
  if (!isTauri) return;
  unlistenProgress = await listen<BiographyProgress>("biography://progress", (event) => {
    const payload = event.payload;
    if (
      props.bookId === payload.book_id &&
      props.target?.kind === payload.kind &&
      props.target?.key === payload.target_key
    ) {
      bioProgress.value = payload;
    }
  });
});

onBeforeUnmount(() => {
  unlistenProgress?.();
});

const kindLabel = computed(() => {
  const kind = props.target?.kind;
  return kind === "character"
    ? "人物"
    : kind === "space"
      ? "地点"
      : kind === "skill"
        ? "功法"
        : kind === "legacy"
          ? "传承"
          : "物品";
});

const timeline = computed(() =>
  [...(props.target?.events ?? [])].sort(
    (a, b) => a.chapter_index - b.chapter_index || a.id - b.id,
  ),
);

const wordCount = computed(() =>
  (biography.value?.content ?? "").replace(/\s/g, "").length,
);

async function loadBiography() {
  biography.value = null;
  if (!props.open || !props.target || !props.bookId) return;
  tab.value = "biography";
  loading.value = true;
  try {
    biography.value = await api.getBiography(props.bookId, props.target.kind, props.target.key);
  } catch (e) {
    ui.toast(`读取传记失败：${errText(e)}`, "error");
  } finally {
    loading.value = false;
  }
}

watch(
  () => [props.open, props.target?.kind, props.target?.key, props.bookId],
  loadBiography,
  { immediate: true },
);

async function generateBiography() {
  if (!props.target || !props.bookId || generating.value) return;
  if (!llm.config.model) {
    ui.toast("请先在「引擎设置」中选择模型", "error");
    return;
  }
  generating.value = true;
  bioProgress.value = null;
  try {
    biography.value = await api.generateBiography(
      props.bookId,
      props.target.kind,
      props.target.key,
      llm.config,
    );
    library.addBiographyKey(props.target.kind, props.target.key);
    ui.toast("传记已生成", "success");
  } catch (e) {
    ui.toast(`传记生成失败：${errText(e)}`, "error");
  } finally {
    generating.value = false;
  }
}
</script>

<template>
  <transition name="fade">
    <div
      v-if="open && target"
      class="fixed inset-0 z-50 grid place-items-center bg-ink-900/75 p-4 backdrop-blur-sm"
      @click.self="emit('close')"
    >
      <div class="panel flex max-h-[90vh] w-full max-w-3xl flex-col overflow-hidden">
        <header class="flex items-start gap-3 border-b border-ink-600/60 px-5 py-4">
          <div class="grid h-10 w-10 shrink-0 place-items-center rounded-lg bg-jade-500/10 text-jade-400">
            <i class="fa-solid" :class="target.icon || 'fa-circle-info'" />
          </div>
          <div class="min-w-0 flex-1">
            <div class="flex flex-wrap items-center gap-2">
              <h3 class="truncate font-serif text-lg font-semibold text-slate-100">{{ target.title }}</h3>
              <span class="rounded-full border border-ink-500/60 px-2 py-0.5 text-[10px] text-slate-400">
                {{ kindLabel }}
              </span>
            </div>
            <p v-if="target.subtitle" class="mt-0.5 truncate text-xs text-slate-400">{{ target.subtitle }}</p>
          </div>
          <button class="btn-ghost !p-2" title="关闭" aria-label="关闭" @click="emit('close')">
            <i class="fa-solid fa-xmark" />
          </button>
        </header>

        <div class="flex shrink-0 gap-1 border-b border-ink-600/60 px-4 pt-3">
          <button
            v-for="item in [
              { key: 'biography', label: '传记', icon: 'fa-feather-pointed' },
              { key: 'metadata', label: '元数据', icon: 'fa-circle-info' },
              { key: 'timeline', label: '时间线', icon: 'fa-timeline' },
            ]"
            :key="item.key"
            class="rounded-t-lg border-b-2 px-3 py-2 text-xs transition"
            :class="
              tab === item.key
                ? 'border-jade-400 text-jade-300'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            "
            @click="tab = item.key as typeof tab"
          >
            <i class="fa-solid mr-1" :class="item.icon" />{{ item.label }}
          </button>
        </div>

        <div class="min-h-0 flex-1 overflow-y-auto px-5 py-4">
          <template v-if="tab === 'biography'">
            <div class="mb-3 flex flex-wrap items-center gap-2">
              <button
                class="btn-ghost !px-2.5 !py-1.5 text-xs"
                :disabled="generating"
                @click="generateBiography"
              >
                <i
                  class="fa-solid"
                  :class="generating ? 'fa-spinner fa-spin' : biography ? 'fa-rotate' : 'fa-feather-pointed'"
                />
                {{ biography ? "重新生成传记" : "生成传记" }}
              </button>
              <span v-if="biography" class="text-[11px] text-slate-500">
                {{ wordCount }} 字 · {{ biography.updated_at }}
              </span>
              <span v-if="generating" class="text-xs text-gold-400">
                <i class="fa-solid fa-spinner fa-spin mr-1" />
                {{ bioProgress?.message || "本地模型生成中…" }}
                <template v-if="bioProgress && bioProgress.total">
                  {{ bioProgress.current }}/{{ bioProgress.total }}
                </template>
              </span>
            </div>

            <div v-if="loading" class="py-10 text-center text-sm text-slate-500">
              <i class="fa-solid fa-spinner fa-spin mr-1" />读取传记…
            </div>
            <p
              v-else-if="biography"
              class="max-h-[58vh] overflow-y-auto whitespace-pre-line pr-2 text-sm leading-relaxed text-slate-300"
            >
              {{ biography.content }}
            </p>
            <p v-else class="py-10 text-center text-sm text-slate-500">
              尚未生成传记。点击上方按钮，使用本地模型根据已抽取资料生成。
            </p>
          </template>

          <template v-else-if="tab === 'metadata'">
            <dl class="grid gap-2 sm:grid-cols-2">
              <div
                v-for="item in target.metadata"
                :key="item.label"
                class="rounded-lg border border-ink-600/50 bg-ink-900/30 px-3 py-2"
              >
                <dt class="text-[10px] uppercase tracking-wide text-slate-500">{{ item.label }}</dt>
                <dd class="mt-0.5 whitespace-pre-line text-sm text-slate-300">{{ item.value || "—" }}</dd>
              </div>
            </dl>
          </template>

          <template v-else>
            <ol v-if="timeline.length" class="relative ml-2 space-y-3 border-l border-ink-500/60 pl-5">
              <li v-for="event in timeline" :key="event.id" class="relative">
                <span class="absolute -left-[25px] top-1.5 h-2 w-2 rounded-full bg-jade-400 ring-4 ring-ink-800" />
                <div class="rounded-lg border border-ink-600/50 bg-ink-900/30 px-3 py-2">
                  <div class="flex flex-wrap items-center gap-2 text-xs">
                    <span class="font-medium text-slate-200">
                      第 {{ event.chapter_index + 1 }} 章 · {{ event.chapter_title }}
                    </span>
                    <span class="rounded border border-ink-500/60 px-1.5 py-0.5 text-[10px] text-gold-400">
                      {{ event.action }}
                    </span>
                    <span v-if="event.time_type" class="text-[10px] text-slate-500">
                      {{ event.time_type }}
                      <template v-if="event.time_offset_days"> · {{ event.time_offset_days }} 天</template>
                    </span>
                  </div>
                  <div v-if="event.role || event.place" class="mt-1 flex flex-wrap gap-x-3 gap-y-0.5 text-[11px] text-slate-400">
                    <span v-if="event.role">角色：{{ event.role }}</span>
                    <span v-if="event.place"><i class="fa-solid fa-location-dot mr-1" />{{ event.place }}</span>
                  </div>
                  <p v-if="event.detail" class="mt-1 text-xs leading-relaxed text-slate-400">{{ event.detail }}</p>
                </div>
              </li>
            </ol>
            <p v-else class="py-10 text-center text-sm text-slate-500">暂无时间线事件。</p>
          </template>
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
