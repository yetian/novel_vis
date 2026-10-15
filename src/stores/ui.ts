import { defineStore } from "pinia";

export type ViewKey = "library" | "canvas" | "settings";
export type ToastKind = "info" | "success" | "error";

export interface Toast {
  id: number;
  kind: ToastKind;
  message: string;
}

export interface ConfirmRequest {
  title: string;
  message: string;
  confirmLabel: string;
  cancelLabel: string;
  danger: boolean;
  resolve: (value: boolean) => void;
}

let toastSeed = 0;

export const useUiStore = defineStore("ui", {
  state: () => ({
    activeView: "library" as ViewKey,
    toasts: [] as Toast[],
    sidebarOpen: true,
    confirm: null as ConfirmRequest | null,
  }),
  actions: {
    setView(view: ViewKey) {
      this.activeView = view;
    },
    /// 弹出确认框，返回用户是否确认。用于删除等破坏性操作。
    askConfirm(options: {
      title: string;
      message: string;
      confirmLabel?: string;
      cancelLabel?: string;
      danger?: boolean;
    }): Promise<boolean> {
      return new Promise((resolve) => {
        this.confirm = {
          title: options.title,
          message: options.message,
          confirmLabel: options.confirmLabel ?? "确认",
          cancelLabel: options.cancelLabel ?? "取消",
          danger: options.danger ?? false,
          resolve,
        };
      });
    },
    resolveConfirm(value: boolean) {
      this.confirm?.resolve(value);
      this.confirm = null;
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
