import { defineStore } from "pinia";
import { api } from "@/api/tauri";
import { errText } from "@/utils/error";
import type { BookInspection, BookRow, TimelineBundle } from "@/types/schema";
import { useUiStore, type ViewKey } from "./ui";

export interface PendingImport {
  path: string;
  inspection: BookInspection;
}

export const useLibraryStore = defineStore("library", {
  state: () => ({
    books: [] as BookRow[],
    activeBookId: null as number | null,
    timeline: null as TimelineBundle | null,
    loading: false,
    ingesting: false,
    pending: null as PendingImport | null,
  }),
  getters: {
    activeBook(state): BookRow | null {
      return state.books.find((b) => b.id === state.activeBookId) ?? null;
    },
    totalChapters(state): number {
      return state.books.reduce((sum, b) => sum + b.total_chapters, 0);
    },
  },
  actions: {
    async refresh() {
      const ui = useUiStore();
      this.loading = true;
      try {
        this.books = await api.listBooks();
        if (this.activeBookId && !this.books.some((b) => b.id === this.activeBookId)) {
          this.activeBookId = null;
          this.timeline = null;
        }
      } catch (e) {
        ui.toast(`读取书库失败：${errText(e)}`, "error");
      } finally {
        this.loading = false;
      }
    },
    /// 导入前先探查；疑似合订本时挂起并请求用户确认。
    async requestIngest(path: string) {
      const ui = useUiStore();
      this.ingesting = true;
      try {
        const inspection = await api.inspectBook(path);
        if (inspection.suspected_multi_book && inspection.candidates.length > 0) {
          this.pending = { path, inspection };
          return;
        }
        await this.doIngest(path, false);
      } catch (e) {
        ui.toast(`导入失败：${errText(e)}`, "error");
      } finally {
        this.ingesting = false;
      }
    },
    async doIngest(path: string, splitIntoBooks: boolean, candidateIds?: string[]) {
      const ui = useUiStore();
      this.ingesting = true;
      try {
        const results = await api.ingestBook(path, splitIntoBooks, candidateIds);
        if (results.length === 1) {
          ui.toast(`已导入《${results[0].title}》，共 ${results[0].total_chapters} 章`, "success");
        } else {
          ui.toast(`已拆分为 ${results.length} 本导入`, "success");
        }
        this.pending = null;
        await this.refresh();
        if (results.length) await this.open(results[0].book_id);
        return results;
      } catch (e) {
        ui.toast(`导入失败：${errText(e)}`, "error");
        return null;
      } finally {
        this.ingesting = false;
      }
    },
    async confirmPending(splitIntoBooks: boolean, candidateIds?: string[]) {
      if (!this.pending) return;
      await this.doIngest(this.pending.path, splitIntoBooks, candidateIds);
    },
    cancelPending() {
      this.pending = null;
    },
    /// 打开书籍；可选在数据加载完成后切换视图（避免先切页时显示上一本书的数据）。
    async open(bookId: number, view?: ViewKey) {
      const ui = useUiStore();
      this.activeBookId = bookId;
      await this.reloadTimeline();
      if (!this.timeline || this.timeline.chapters.length === 0) {
        ui.toast("该书尚未抽取任何章节", "info");
      }
      if (view) ui.setView(view);
    },
    async reloadTimeline() {
      if (!this.activeBookId) return;
      try {
        this.timeline = await api.getTimeline(this.activeBookId);
      } catch (e) {
        useUiStore().toast(`读取时空数据失败：${errText(e)}`, "error");
      }
    },
    async remove(bookId: number) {
      const ui = useUiStore();
      const book = this.books.find((b) => b.id === bookId);
      const ok = await ui.askConfirm({
        title: "删除书籍",
        message: `确认删除《${book?.title ?? bookId}》及其全部抽取数据？\n此操作不可撤销。`,
        confirmLabel: "删除",
        danger: true,
      });
      if (!ok) return;
      try {
        await api.deleteBook(bookId);
        ui.toast("已删除书籍及其抽取数据", "success");
        if (this.activeBookId === bookId) {
          this.activeBookId = null;
          this.timeline = null;
        }
        await this.refresh();
      } catch (e) {
        ui.toast(`删除失败：${errText(e)}`, "error");
      }
    },
  },
});
