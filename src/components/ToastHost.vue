<script setup lang="ts">
import { useUiStore } from "@/stores/ui";

const ui = useUiStore();

const iconFor = (kind: string) =>
  kind === "success" ? "fa-circle-check" : kind === "error" ? "fa-circle-exclamation" : "fa-circle-info";
const classFor = (kind: string) =>
  kind === "success"
    ? "border-jade-500/50 text-jade-400"
    : kind === "error"
      ? "border-cinnabar-500/50 text-cinnabar-400"
      : "border-ink-500 text-slate-300";
</script>

<template>
  <div class="pointer-events-none fixed bottom-4 right-4 z-50 flex w-80 flex-col gap-2">
    <transition-group name="toast">
      <div
        v-for="t in ui.toasts"
        :key="t.id"
        class="pointer-events-auto flex items-start gap-3 rounded-lg border bg-ink-800/95 px-4 py-3 text-sm shadow-lg backdrop-blur"
        :class="classFor(t.kind)"
      >
        <i class="fa-solid mt-0.5" :class="iconFor(t.kind)" />
        <span class="flex-1 text-slate-200">{{ t.message }}</span>
        <button class="text-slate-500 hover:text-slate-200" @click="ui.dismiss(t.id)">
          <i class="fa-solid fa-xmark" />
        </button>
      </div>
    </transition-group>
  </div>
</template>

<style scoped>
.toast-enter-active,
.toast-leave-active {
  transition: all 0.25s ease;
}
.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateX(20px);
}
</style>
