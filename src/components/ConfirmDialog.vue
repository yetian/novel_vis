<script setup lang="ts">
import { computed } from "vue";
import { useUiStore } from "@/stores/ui";

const ui = useUiStore();
const req = computed(() => ui.confirm);
</script>

<template>
  <transition name="fade">
    <div
      v-if="req"
      class="fixed inset-0 z-50 grid place-items-center bg-ink-900/70 p-4 backdrop-blur-sm"
      @click.self="ui.resolveConfirm(false)"
      @keydown.esc="ui.resolveConfirm(false)"
    >
      <div class="panel w-full max-w-md overflow-hidden">
        <div class="flex items-start gap-3 px-5 py-4">
          <div
            class="grid h-10 w-10 shrink-0 place-items-center rounded-lg"
            :class="req.danger ? 'bg-cinnabar-500/15 text-cinnabar-400' : 'bg-jade-500/15 text-jade-400'"
          >
            <i class="fa-solid" :class="req.danger ? 'fa-triangle-exclamation' : 'fa-circle-question'" />
          </div>
          <div class="min-w-0">
            <h3 class="font-semibold text-slate-100">{{ req.title }}</h3>
            <p class="mt-1 whitespace-pre-line text-sm text-slate-400">{{ req.message }}</p>
          </div>
        </div>
        <div class="flex justify-end gap-2 border-t border-ink-600/60 px-5 py-4">
          <button class="btn-ghost" @click="ui.resolveConfirm(false)">{{ req.cancelLabel }}</button>
          <button :class="req.danger ? 'btn-danger' : 'btn-primary'" @click="ui.resolveConfirm(true)">
            {{ req.confirmLabel }}
          </button>
        </div>
      </div>
    </div>
  </transition>
</template>

<style scoped>
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.18s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
