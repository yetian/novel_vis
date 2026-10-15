import { defineStore } from "pinia";
import { api } from "@/api/tauri";
import type { EngineKind, LlmConfig, ModelInfo } from "@/types/schema";
import { ENGINE_PRESETS } from "@/types/schema";
import { useUiStore } from "./ui";

function defaultConfig(): LlmConfig {
  return {
    engine: "lm_studio",
    base_url: ENGINE_PRESETS.lm_studio.baseUrl,
    api_key: null,
    model: "qwen2.5-1.5b-instruct",
    temperature: 0.1,
    max_tokens: 2048,
    timeout_secs: 180,
    max_retries: 3,
  };
}

export const useLlmStore = defineStore("llm", {
  state: () => ({
    config: defaultConfig(),
    models: [] as ModelInfo[],
    loading: false,
    connected: false,
    lastError: "" as string,
  }),
  getters: {
    engineLabel(state): string {
      return ENGINE_PRESETS[state.config.engine]?.label ?? state.config.engine;
    },
    recommendedChips(state): ModelInfo[] {
      const needles = ["qwen2.5-0.5b", "qwen2.5-1.5b", "qwen2.5-3b", "gemma-2-2b", "gemma"];
      return state.models
        .filter((m) => needles.some((n) => m.id.toLowerCase().includes(n)))
        .slice(0, 6);
    },
  },
  actions: {
    applyEngine(engine: EngineKind) {
      this.config.engine = engine;
      this.config.base_url = ENGINE_PRESETS[engine].baseUrl;
    },
    async load() {
      const ui = useUiStore();
      try {
        const saved = await api.getLlmConfig();
        if (saved) this.config = { ...this.config, ...saved };
      } catch {
        // 首次运行或非 Tauri 环境：使用默认配置
        void ui;
      }
    },
    async persist() {
      const ui = useUiStore();
      try {
        await api.saveLlmConfig(this.config);
        ui.toast(`已保存 ${this.engineLabel} 配置`, "success");
      } catch (e) {
        ui.toast(`保存配置失败：${String(e)}`, "error");
      }
    },
    async fetchModels() {
      const ui = useUiStore();
      this.loading = true;
      this.lastError = "";
      try {
        const models = await api.listModels(this.config);
        this.models = models;
        this.connected = true;
        if (!models.some((m) => m.id === this.config.model) && models.length) {
          this.config.model = models[0].id;
        }
        ui.toast(`发现 ${models.length} 个可用模型`, "success");
      } catch (e) {
        this.connected = false;
        this.lastError = String(e);
        ui.toast(`无法获取模型列表：${String(e)}`, "error");
      } finally {
        this.loading = false;
      }
    },
    async test() {
      const ui = useUiStore();
      this.loading = true;
      try {
        const ok = await api.testConnection(this.config);
        this.connected = ok;
        ui.toast(ok ? "连接成功" : "连接失败", ok ? "success" : "error");
      } catch (e) {
        this.connected = false;
        ui.toast(`连接失败：${String(e)}`, "error");
      } finally {
        this.loading = false;
      }
    },
  },
});
