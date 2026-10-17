<script setup lang="ts">
import { onMounted, onBeforeUnmount } from "vue";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { UnlistenFn } from "@tauri-apps/api/event";
import Sidebar from "@/components/Sidebar.vue";
import LibraryView from "@/components/LibraryView.vue";
import SummaryView from "@/components/SummaryView.vue";
import SpacetimeCanvas from "@/components/SpacetimeCanvas.vue";
import RelationGraph from "@/components/RelationGraph.vue";
import EntityCards from "@/components/EntityCards.vue";
import LlmSettings from "@/components/LlmSettings.vue";
import MultiBookDialog from "@/components/MultiBookDialog.vue";
import ConfirmDialog from "@/components/ConfirmDialog.vue";
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
  { key: "summary", label: "总览", icon: "fa-list-ul" },
  { key: "canvas", label: "时空画布", icon: "fa-diagram-project" },
  { key: "relations", label: "人物关系", icon: "fa-users" },
  { key: "cards", label: "空间物品", icon: "fa-table-cells-large" },
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
          await library.requestIngest(path);
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
    <header class="flex items-center gap-4 border-b border-ink-500/40 bg-ink-800/75 px-5 py-3 backdrop-blur">
      <div class="flex items-center gap-3">
        <div class="grid h-9 w-9 place-items-center rounded-xl border border-jade-500/20 bg-jade-500/15 text-jade-400 shadow-glow">
          <i class="fa-solid fa-timeline text-lg" />
        </div>
        <div>
          <h1 class="font-serif text-[17px] font-semibold leading-tight text-slate-100">时空编年史</h1>
          <p class="text-[11px] text-slate-400">ChronoSpace Reader · 本地叙事结构抽取</p>
        </div>
      </div>

      <nav class="ml-4 flex items-center gap-1 rounded-xl border border-ink-500/30 bg-ink-900/50 p-1">
        <button
          v-for="item in navItems"
          :key="item.key"
          class="btn !px-3 !py-1.5"
          :class="
            ui.activeView === item.key
              ? 'bg-jade-500 text-ink-900 shadow-sm'
              : 'text-slate-300 hover:bg-ink-600/80 hover:text-slate-100'
          "
          @click="ui.setView(item.key)"
        >
          <i class="fa-solid" :class="item.icon" />
          {{ item.label }}
        </button>
      </nav>

      <div class="ml-auto flex items-center gap-3">
        <span class="hidden rounded-md border border-ink-500/50 px-2 py-1 text-[11px] tabular-nums text-slate-400 sm:inline">
          v{{ version }}
        </span>
        <div class="hidden items-center gap-2 rounded-lg border border-ink-500/50 bg-ink-900/30 px-3 py-1.5 text-xs md:flex">
          <span class="h-2 w-2 rounded-full" :class="llm.connected ? 'bg-jade-500' : 'bg-slate-500'" />
          <span class="text-slate-300">{{ llm.engineLabel }}</span>
          <span class="text-slate-600">|</span>
          <span class="max-w-[160px] truncate text-slate-200">{{ llm.config.model || "未选择模型" }}</span>
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
        <SummaryView v-else-if="ui.activeView === 'summary'" />
        <SpacetimeCanvas v-else-if="ui.activeView === 'canvas'" />
        <RelationGraph v-else-if="ui.activeView === 'relations'" />
        <EntityCards v-else-if="ui.activeView === 'cards'" />
        <LlmSettings v-else />
      </main>
    </div>

    <MultiBookDialog />
    <ConfirmDialog />
    <ToastHost />
  </div>
</template>
