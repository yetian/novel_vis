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

const tab = ref<"biography" | "metadata" | "roles" | "timeline">("biography");
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

const tabs = computed(() => [
  { key: "biography", label: "传记", icon: "fa-feather-pointed" },
  { key: "metadata", label: "元数据", icon: "fa-circle-info" },
  ...(props.target?.kind === "character"
    ? [{ key: "roles", label: "身份 / 关系", icon: "fa-id-badge" }]
    : []),
  { key: "timeline", label: "时间线", icon: "fa-timeline" },
]);

const relations = computed(() => props.target?.relations ?? []);
const roles = computed(() => props.target?.roles ?? []);

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
            v-for="item in tabs"
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

          <template v-else-if="tab === 'roles'">
            <section>
              <h4 class="text-xs font-semibold uppercase tracking-wide text-slate-400">
                身份 / 角色（{{ roles.length }}）
              </h4>
              <div
                v-if="roles.length"
                class="relative mt-3 ml-2 space-y-3 border-l border-ink-500/60 pl-5"
              >
                <div
                  v-for="role in roles"
                  :key="role.name"
                  class="relative"
                  :title="`首次获得：第 ${role.first_chapter + 1} 章 ${role.first_chapter_title}${role.first_place ? ` · ${role.first_place}` : ''}；共 ${role.count} 次`"
                >
                  <span
                    class="absolute -left-[25px] top-2 h-2.5 w-2.5 rounded-full ring-4 ring-ink-800"
                    :class="role.level_up ? 'bg-gold-400' : 'bg-jade-400'"
                  />
                  <span
                    v-if="role.level_up"
                    class="absolute -left-[25px] -top-3 h-3 w-px bg-gold-400/80"
                  />
                  <div class="rounded-lg border border-ink-600/50 bg-ink-900/30 px-3 py-2">
                    <div class="flex flex-wrap items-center gap-2">
                      <span class="text-sm font-medium text-slate-100">{{ role.name }}</span>
                      <span
                        v-if="role.level_up"
                        class="rounded-full border border-gold-400/40 bg-gold-400/10 px-1.5 py-0.5 text-[9px] text-gold-300"
                      >
                        身份变化 / 晋升
                      </span>
                      <span class="text-[10px] text-slate-500">{{ role.count }} 次</span>
                    </div>
                    <div class="mt-1 text-[11px] text-slate-400">
                      首次获得：第 {{ role.first_chapter + 1 }} 章
                      <template v-if="role.first_chapter_title"> · {{ role.first_chapter_title }}</template>
                      <template v-if="role.first_place"> · {{ role.first_place }}</template>
                    </div>

                    <details class="mt-2 border-t border-ink-600/40 pt-2">
                      <summary class="cursor-pointer text-[11px] text-slate-400 hover:text-slate-200">
                        查看身份轨迹（{{ role.trace.length }}）
                      </summary>
                      <ol class="mt-2 space-y-1.5 border-l border-ink-500/40 pl-3">
                        <li v-for="trace in role.trace" :key="`${role.name}-${trace.chapter_index}-${trace.action}`">
                          <div class="text-[11px] text-slate-300">
                            第 {{ trace.chapter_index + 1 }} 章
                            <template v-if="trace.chapter_title"> · {{ trace.chapter_title }}</template>
                          </div>
                          <div class="text-[10px] text-slate-500">
                            <template v-if="trace.place">{{ trace.place }} · </template>
                            {{ trace.action }}
                            <template v-if="trace.detail"> · {{ trace.detail }}</template>
                          </div>
                        </li>
                      </ol>
                    </details>
                  </div>
                </div>
              </div>
              <p v-else class="mt-2 text-sm text-slate-500">未记录到身份/角色。</p>
            </section>

            <section class="mt-6">
              <h4 class="text-xs font-semibold uppercase tracking-wide text-slate-400">
                关系解释（{{ relations.length }}）
              </h4>
              <div v-if="relations.length" class="mt-2 grid gap-2 sm:grid-cols-2">
                <div
                  v-for="relation in relations"
                  :key="relation.name"
                  class="rounded-lg border border-ink-600/50 bg-ink-900/30 px-3 py-2"
                  :title="`共同出场 ${relation.weight} 章`"
                >
                  <div class="text-sm text-slate-200">
                    {{ target.title }}（{{ relation.self_role || "—" }}）
                    <span class="mx-1 text-slate-500">·</span>
                    {{ relation.name }}（{{ relation.other_role || "—" }}）
                  </div>
                  <div class="mt-1 text-[11px] text-slate-500">
                    共同出场 {{ relation.weight }} 章 · 第 {{ relation.first_chapter + 1 }}–{{ relation.last_chapter + 1 }} 章
                  </div>
                </div>
              </div>
              <p v-else class="mt-2 text-sm text-slate-500">暂无关系记录。</p>
            </section>
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
