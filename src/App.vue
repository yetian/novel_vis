<script setup lang="ts">
import { onMounted, onBeforeUnmount } from "vue";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { UnlistenFn } from "@tauri-apps/api/event";
import Sidebar from "@/components/Sidebar.vue";
import LibraryView from "@/components/LibraryView.vue";
import SpacetimeCanvas from "@/components/SpacetimeCanvas.vue";
import LlmSettings from "@/components/LlmSettings.vue";
import ToastHost from "@/components/ToastHost.vue";
import { useUiStore, type ViewKey } from "@/stores/ui";
import { useLibraryStore } from "@/stores/library";
import { useLlmStore } from "@/stores/llm";
import { useExtractionStore } from "@/stores/extraction";
import { isTauri } from "@/api/tauri";

const ui = useUiStore();
const library = useLibraryStore();
const llm = useLlmStore();
const extraction = useExtractionStore();

const version = __APP_VERSION__;

const navItems: { key: ViewKey; label: string; icon: string }[] = [
  { key: "library", label: "图书馆", icon: "fa-book-open-reader" },
  { key: "canvas", label: "时空画布", icon: "fa-diagram-project" },
  { key: "settings", label: "引擎设置", icon: "fa-sliders" },
];

let unlisten: UnlistenFn[] = [];

onMounted(async () => {
  await llm.load();
  await library.refresh();
  await extraction.attach();

  if (isTauri) {
    const webview = getCurrentWebview();
    const drop = await webview.onDragDropEvent(async (event) => {
      if (event.payload.type === "drop") {
        for (const path of event.payload.paths) {
          await library.ingest(path);
        }
      }
    });
    unlisten.push(drop);
  }
});

onBeforeUnmount(() => {
  unlisten.forEach((fn) => fn());
  extraction.detach();
});
</script>

<template>
  <div class="flex h-full flex-col">
    <header class="flex items-center gap-4 border-b border-ink-600/60 bg-ink-800/60 px-5 py-3 backdrop-blur">
      <div class="flex items-center gap-3">
        <div class="grid h-9 w-9 place-items-center rounded-lg bg-jade-500/15 text-jade-400 shadow-glow">
          <i class="fa-solid fa-timeline text-lg" />
        </div>
        <div>
          <h1 class="text-base font-semibold leading-tight text-slate-100">时空编年史</h1>
          <p class="text-[11px] text-slate-500">ChronoSpace Reader · 本地叙事结构抽取</p>
        </div>
      </div>

      <nav class="ml-4 flex items-center gap-1 rounded-lg bg-ink-900/60 p-1">
        <button
          v-for="item in navItems"
          :key="item.key"
          class="btn !px-3 !py-1.5"
          :class="ui.activeView === item.key ? 'bg-jade-500 text-ink-900' : 'text-slate-300 hover:bg-ink-600'"
          @click="ui.setView(item.key)"
        >
          <i class="fa-solid" :class="item.icon" />
          {{ item.label }}
        </button>
      </nav>

      <div class="ml-auto flex items-center gap-3">
        <span class="hidden rounded-md border border-ink-600/60 px-2 py-1 text-[11px] text-slate-500 sm:inline">
          v{{ version }}
        </span>
        <div class="hidden items-center gap-2 rounded-lg border border-ink-600/60 px-3 py-1.5 text-xs md:flex">
          <span class="h-2 w-2 rounded-full" :class="llm.connected ? 'bg-jade-500' : 'bg-slate-500'" />
          <span class="text-slate-400">{{ llm.engineLabel }}</span>
          <span class="text-slate-600">|</span>
          <span class="max-w-[160px] truncate text-slate-300">{{ llm.config.model || "未选择模型" }}</span>
        </div>
        <button class="btn-ghost" title="引擎设置" @click="ui.setView('settings')">
          <i class="fa-solid fa-gear" />
        </button>
      </div>
    </header>

    <div class="flex min-h-0 flex-1">
      <Sidebar />
      <main class="min-w-0 flex-1 overflow-hidden">
        <LibraryView v-if="ui.activeView === 'library'" />
        <SpacetimeCanvas v-else-if="ui.activeView === 'canvas'" />
        <LlmSettings v-else />
      </main>
    </div>

    <ToastHost />
  </div>
</template>
