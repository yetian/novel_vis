<script setup lang="ts">
import { computed, onMounted } from "vue";
import { useLibraryStore } from "@/stores/library";
import { useExtractionStore, type BookJob } from "@/stores/extraction";
import { useLlmStore } from "@/stores/llm";
import { useUiStore } from "@/stores/ui";
import { chapterLabel } from "@/utils/chapter";

const library = useLibraryStore();
const extraction = useExtractionStore();
const llm = useLlmStore();
const ui = useUiStore();

const book = computed(() => library.activeBook);
const bookId = computed(() => book.value?.id ?? null);
const job = computed<BookJob | null>(() => extraction.jobFor(bookId.value));

const running = computed(() => job.value?.running ?? false);
const paused = computed(() => job.value?.paused ?? false);
const percent = computed(() => extraction.percent(bookId.value));
const okCount = computed(() => job.value?.results.filter((r) => r.ok).length ?? 0);
const failedCount = computed(() => job.value?.results.filter((r) => !r.ok).length ?? 0);

const phaseLabel: Record<string, string> = {
  extracting: "抽取中",
  paused: "已暂停",
  skipped: "跳过",
  chapter_done: "章节完成",
  summarizing: "汇总全书",
  finished: "已完成",
  cancelled: "已取消",
};

async function start() {
  if (!bookId.value) return;
  if (!llm.config.model) {
    ui.toast("请先在「引擎设置」中选择模型", "error");
    return;
  }
  // 关闭「跳过已完成」会重新处理整本书，并覆盖已有抽取数据。
  if (!extraction.skipExtracted && (book.value?.extracted_chapters ?? 0) > 0) {
    const ok = await ui.askConfirm({
      title: "将重新抽取整本书",
      message: `「跳过已完成」已关闭，将重新处理整本书，并删除该书已有的抽取数据（${book.value?.extracted_chapters} 章）。\n此操作不可撤销，确定继续吗？`,
      confirmLabel: "删除并重新抽取",
      danger: true,
    });
    if (!ok) return;
  }
  extraction.start(bookId.value);
}

onMounted(() => {
  llm.fetchGpu().catch(() => undefined);
});

const gpuSuggestion = computed(() => llm.gpu?.suggested_concurrency ?? 1);

async function cancel() {
  if (!bookId.value) return;
  const ok = await ui.askConfirm({
    title: "取消抽取",
    message: `确认取消《${book.value?.title}》的抽取任务？已完成章节的进度会保留。`,
    confirmLabel: "取消任务",
    danger: true,
  });
  if (ok) extraction.cancel(bookId.value);
}
</script>

<template>
  <section v-if="book" class="panel p-5">
    <div class="flex flex-wrap items-center gap-3">
      <div class="flex items-center gap-2">
        <i class="fa-solid fa-wand-magic-sparkles text-gold-400" />
        <h3 class="font-semibold text-slate-100">批量抽取 · {{ book.title }}</h3>
      </div>
      <span class="rounded-full border border-ink-500 px-2 py-0.5 text-[11px] text-slate-400">
        {{ llm.engineLabel }} / {{ llm.config.model || "未选择模型" }}
      </span>

      <div class="ml-auto flex flex-wrap items-center gap-2">
        <label class="flex items-center gap-1.5 text-xs" :title="'关闭后将重新处理整本书，并删除已有的抽取数据'">
          <input v-model="extraction.skipExtracted" type="checkbox" class="accent-jade-500" />
          <span :class="extraction.skipExtracted ? 'text-slate-400' : 'text-gold-400'">
            跳过已完成{{ extraction.skipExtracted ? "" : "（将清空重抽）" }}
          </span>
        </label>
        <label class="flex items-center gap-1.5 text-xs" title="整章输入：不切块，将整章文本直接交给模型（需足够的上下文窗口）">
          <input v-model="extraction.wholeChapter" type="checkbox" class="accent-gold-400" />
          <span :class="extraction.wholeChapter ? 'text-gold-400' : 'text-slate-400'">整章</span>
        </label>
        <label class="flex items-center gap-1.5 text-xs text-slate-400">
          分块
          <input
            v-model.number="extraction.chunkChars"
            type="number"
            min="800"
            step="500"
            :disabled="extraction.wholeChapter"
            class="w-20 rounded border border-ink-500 bg-ink-900 px-2 py-1 text-slate-200 disabled:opacity-40"
          />
        </label>
        <label class="flex items-center gap-1.5 text-xs text-slate-400" title="并发请求数（1-4）。需引擎支持并行/批处理才会提速">
          并发
          <select v-model.number="extraction.concurrency" class="rounded border border-ink-500 bg-ink-900 px-2 py-1 text-slate-200">
            <option v-for="n in 4" :key="n" :value="n">{{ n }}</option>
          </select>
        </label>
        <span
          v-if="gpuSuggestion > 1"
          class="cursor-help text-[11px] text-gold-400"
          title="依据模型显存占用估计，可允许更多并发"
          @click="extraction.concurrency = gpuSuggestion"
        >
          <i class="fa-solid fa-microchip mr-1" />建议 {{ gpuSuggestion }} 路
        </span>

        <button v-if="!running" class="btn-primary" @click="start">
          <i class="fa-solid fa-play" />开始抽取
        </button>
        <template v-else>
          <button v-if="!paused" class="btn-ghost" @click="extraction.pause(bookId!)">
            <i class="fa-solid fa-pause" />暂停
          </button>
          <button v-else class="btn-primary" @click="extraction.resume(bookId!)">
            <i class="fa-solid fa-play" />继续
          </button>
          <button class="btn-danger" @click="cancel">
            <i class="fa-solid fa-stop" />取消
          </button>
        </template>
      </div>
    </div>

    <!-- 进度 -->
    <div class="mt-4 space-y-2">
      <div class="flex items-center justify-between text-xs text-slate-400">
        <span>
          <i class="fa-solid fa-circle-notch mr-1" :class="{ 'fa-spin': running }" />
          {{ job?.progress ? phaseLabel[job.progress.phase] || job.progress.phase : "待开始" }}
          <span v-if="job?.progress?.chapter_title" class="text-slate-500">
            · {{ chapterLabel({ title: job.progress.chapter_title, chapter_index: job.progress.chapter_index }, book?.title) }}
          </span>
        </span>
        <span>{{ percent }}%</span>
      </div>
      <div class="h-2 overflow-hidden rounded-full bg-ink-600">
        <div
          class="h-full rounded-full bg-gradient-to-r from-jade-500 to-gold-400 transition-all duration-300"
          :style="{ width: percent + '%' }"
        />
      </div>
      <p v-if="job?.progress?.message" class="text-[11px] text-slate-500">
        {{ job.progress.message }}
        <span v-if="job.progress.retries" class="text-cinnabar-400">（重试 {{ job.progress.retries }}）</span>
      </p>
    </div>

    <!-- 结果概览 -->
    <div v-if="job?.results.length" class="mt-4 grid grid-cols-3 gap-3 text-center">
      <div class="rounded-lg bg-ink-700/50 p-3">
        <div class="text-lg font-semibold text-slate-100">{{ job.results.length }}</div>
        <div class="text-[11px] text-slate-500">处理章节</div>
      </div>
      <div class="rounded-lg bg-ink-700/50 p-3">
        <div class="text-lg font-semibold text-jade-400">{{ okCount }}</div>
        <div class="text-[11px] text-slate-500">成功</div>
      </div>
      <div class="rounded-lg bg-ink-700/50 p-3">
        <div class="text-lg font-semibold text-cinnabar-400">{{ failedCount }}</div>
        <div class="text-[11px] text-slate-500">失败</div>
      </div>
    </div>

    <!-- 日志 -->
    <details v-if="job?.logs.length" class="mt-4">
      <summary class="cursor-pointer text-xs text-slate-400">运行日志 ({{ job.logs.length }})</summary>
      <pre class="mt-2 max-h-40 overflow-auto rounded-lg bg-ink-900/80 p-3 text-[11px] leading-relaxed text-slate-400">{{
        job.logs.join("\n")
      }}</pre>
    </details>
  </section>
</template>
