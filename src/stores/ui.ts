import { defineStore } from "pinia";

export type ViewKey = "library" | "canvas" | "settings";
export type ToastKind = "info" | "success" | "error";

export interface Toast {
  id: number;
  kind: ToastKind;
  message: string;
}

let toastSeed = 0;

export const useUiStore = defineStore("ui", {
  state: () => ({
    activeView: "library" as ViewKey,
    toasts: [] as Toast[],
    sidebarOpen: true,
  }),
  actions: {
    setView(view: ViewKey) {
      this.activeView = view;
    },
    toast(message: string, kind: ToastKind = "info") {
      const id = ++toastSeed;
      this.toasts.push({ id, kind, message });
      setTimeout(() => this.dismiss(id), 4200);
    },
    dismiss(id: number) {
      this.toasts = this.toasts.filter((t) => t.id !== id);
    },
  },
});
