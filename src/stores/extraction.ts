import { defineStore } from "pinia";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { api } from "@/api/tauri";
import type { ChapterExtractionResult, ProgressEvent } from "@/types/schema";
import { useLlmStore } from "./llm";
import { useLibraryStore } from "./library";
import { useUiStore } from "./ui";

export const useExtractionStore = defineStore("extraction", {
  state: () => ({
    running: false,
    paused: false,
    progress: null as ProgressEvent | null,
    logs: [] as string[],
    results: [] as ChapterExtractionResult[],
    bookId: null as number | null,
    skipExtracted: true,
    chunkChars: 3000,
    unlisten: [] as UnlistenFn[],
  }),
  getters: {
    percent(state): number {
      if (!state.progress || !state.progress.total) return 0;
      return Math.min(100, Math.round((state.progress.current / state.progress.total) * 100));
    },
    okCount(state): number {
      return state.results.filter((r) => r.ok).length;
    },
    failedCount(state): number {
      return state.results.filter((r) => !r.ok).length;
    },
  },
  actions: {
    async attach() {
      if (this.unlisten.length) return;
      const progress = await listen<ProgressEvent>("extraction://progress", (event) => {
        this.progress = event.payload;
        if (event.payload.phase === "finished" || event.payload.phase === "cancelled") {
          this.running = false;
          this.paused = false;
        }
      });
      const log = await listen<string>("extraction://log", (event) => {
        this.logs.unshift(event.payload);
        if (this.logs.length > 300) this.logs.pop();
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
      await this.attach();
      this.bookId = bookId;
      this.running = true;
      this.paused = false;
      this.results = [];
      this.logs = [];
      try {
        this.results = await api.startExtraction({
          book_id: bookId,
          llm: llm.config,
          chunk_chars: this.chunkChars,
          skip_extracted: this.skipExtracted,
        });
        const ok = this.results.filter((r) => r.ok).length;
        ui.toast(`抽取完成：成功 ${ok}/${this.results.length} 章`, ok === this.results.length ? "success" : "info");
        await useLibraryStore().reloadTimeline();
        await useLibraryStore().refresh();
      } catch (e) {
        ui.toast(`抽取失败：${String(e)}`, "error");
      } finally {
        this.running = false;
        this.paused = false;
      }
    },
    async pause() {
      if (!this.bookId) return;
      await api.pauseExtraction(this.bookId);
      this.paused = true;
      useUiStore().toast("已暂停，进度已保存", "info");
    },
    async resume() {
      if (!this.bookId) return;
      await api.resumeExtraction(this.bookId);
      this.paused = false;
    },
    async cancel() {
      if (!this.bookId) return;
      await api.cancelExtraction(this.bookId);
      useUiStore().toast("正在取消任务…", "info");
    },
  },
});
