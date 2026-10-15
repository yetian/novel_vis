import { defineStore } from "pinia";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { api } from "@/api/tauri";
import type { ChapterExtractionResult, LogEvent, ProgressEvent } from "@/types/schema";
import { useLlmStore } from "./llm";
import { useLibraryStore } from "./library";
import { useUiStore } from "./ui";

/// 每本书独立的抽取任务状态，支持「暂停 A 本、同时抽取 B 本」。
export interface BookJob {
  bookId: number;
  running: boolean;
  paused: boolean;
  progress: ProgressEvent | null;
  logs: string[];
  results: ChapterExtractionResult[];
}

function newJob(bookId: number): BookJob {
  return { bookId, running: false, paused: false, progress: null, logs: [], results: [] };
}

export const useExtractionStore = defineStore("extraction", {
  state: () => ({
    jobs: {} as Record<number, BookJob>,
    skipExtracted: true,
    chunkChars: 3000,
    unlisten: [] as UnlistenFn[],
  }),
  getters: {
    jobFor: (state) => (bookId: number | null): BookJob | null =>
      bookId == null ? null : (state.jobs[bookId] ?? null),
    runningBookIds: (state): number[] =>
      Object.values(state.jobs)
        .filter((j) => j.running)
        .map((j) => j.bookId),
    percent: (state) => (bookId: number | null): number => {
      const job = bookId == null ? null : state.jobs[bookId];
      if (!job?.progress || !job.progress.total) return 0;
      return Math.min(100, Math.round((job.progress.current / job.progress.total) * 100));
    },
  },
  actions: {
    ensure(bookId: number): BookJob {
      if (!this.jobs[bookId]) this.jobs[bookId] = newJob(bookId);
      return this.jobs[bookId];
    },
    async attach() {
      if (this.unlisten.length) return;
      const progress = await listen<ProgressEvent>("extraction://progress", (event) => {
        const job = this.ensure(event.payload.book_id);
        job.progress = event.payload;
        if (event.payload.phase === "finished" || event.payload.phase === "cancelled") {
          job.running = false;
          job.paused = false;
        }
      });
      const log = await listen<LogEvent>("extraction://log", (event) => {
        const job = this.ensure(event.payload.book_id);
        job.logs.unshift(event.payload.message);
        if (job.logs.length > 300) job.logs.pop();
      });
      this.unlisten.push(progress, log);
    },
    detach() {
      this.unlisten.forEach((fn) => fn());
      this.unlisten = [];
    },
    async start(bookId: number) {
      const ui = useUiStore();
      const llm = useLlmStore();
      if (!llm.config.model) {
        ui.toast("请先在设置中选择模型", "error");
        return;
      }
      const job = this.ensure(bookId);
      if (job.running) {
        ui.toast("该书正在抽取中", "info");
        return;
      }
      await this.attach();

      job.running = true;
      job.paused = false;
      job.results = [];
      job.logs = [];
      job.progress = null;

      try {
        job.results = await api.startExtraction({
          book_id: bookId,
          llm: llm.config,
          chunk_chars: this.chunkChars,
          skip_extracted: this.skipExtracted,
        });
        const ok = job.results.filter((r) => r.ok).length;
        ui.toast(
          `《${useLibraryStore().books.find((b) => b.id === bookId)?.title ?? bookId}》抽取完成：成功 ${ok}/${job.results.length} 章`,
          ok === job.results.length ? "success" : "info",
        );
        const library = useLibraryStore();
        if (library.activeBookId === bookId) await library.reloadTimeline();
        await library.refresh();
      } catch (e) {
        ui.toast(`抽取失败：${String(e)}`, "error");
      } finally {
        job.running = false;
        job.paused = false;
      }
    },
    async pause(bookId: number) {
      await api.pauseExtraction(bookId);
      const job = this.ensure(bookId);
      job.paused = true;
      useUiStore().toast("已暂停，进度已保存", "info");
    },
    async resume(bookId: number) {
      await api.resumeExtraction(bookId);
      this.ensure(bookId).paused = false;
    },
    async cancel(bookId: number) {
      await api.cancelExtraction(bookId);
      useUiStore().toast("正在取消任务…", "info");
    },
  },
});
