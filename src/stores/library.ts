import { defineStore } from "pinia";
import { api } from "@/api/tauri";
import type { BookRow, TimelineBundle } from "@/types/schema";
import { useUiStore } from "./ui";

export const useLibraryStore = defineStore("library", {
  state: () => ({
    books: [] as BookRow[],
    activeBookId: null as number | null,
    timeline: null as TimelineBundle | null,
    loading: false,
    ingesting: false,
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
        ui.toast(`读取书库失败：${String(e)}`, "error");
      } finally {
        this.loading = false;
      }
    },
    async ingest(path: string) {
      const ui = useUiStore();
      this.ingesting = true;
      try {
        const result = await api.ingestBook(path);
        ui.toast(`已导入《${result.title}》，共 ${result.total_chapters} 章`, "success");
        await this.refresh();
        await this.open(result.book_id);
        return result;
      } catch (e) {
        ui.toast(`导入失败：${String(e)}`, "error");
        return null;
      } finally {
        this.ingesting = false;
      }
    },
    async open(bookId: number) {
      const ui = useUiStore();
      this.activeBookId = bookId;
      await this.reloadTimeline();
      if (!this.timeline || this.timeline.chapters.length === 0) {
        ui.toast("该书尚未抽取任何章节", "info");
      }
    },
    async reloadTimeline() {
      if (!this.activeBookId) return;
      try {
        this.timeline = await api.getTimeline(this.activeBookId);
      } catch (e) {
        useUiStore().toast(`读取时空数据失败：${String(e)}`, "error");
      }
    },
    async remove(bookId: number) {
      const ui = useUiStore();
      try {
        await api.deleteBook(bookId);
        ui.toast("已删除书籍及其抽取数据", "success");
        if (this.activeBookId === bookId) {
          this.activeBookId = null;
          this.timeline = null;
        }
        await this.refresh();
      } catch (e) {
        ui.toast(`删除失败：${String(e)}`, "error");
      }
    },
  },
});
