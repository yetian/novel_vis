<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useLlmStore } from "@/stores/llm";
import { useLibraryStore } from "@/stores/library";
import { useUiStore } from "@/stores/ui";
import { api, isTauri } from "@/api/tauri";
import { errText } from "@/utils/error";
import { ENGINE_PRESETS, MODEL_CHIPS, type EngineKind } from "@/types/schema";

const llm = useLlmStore();
const library = useLibraryStore();
const ui = useUiStore();
const dataDir = ref("");

async function clearAllData() {
  const ok = await ui.askConfirm({
    title: "清空全部数据",
    message: "确认删除所有书籍与抽取结果？此操作不可恢复，需重新导入并抽取。",
    confirmLabel: "清空",
    danger: true,
  });
  if (!ok) return;
  try {
    await api.clearAllData();
    await library.refresh();
    library.activeBookId = null;
    library.timeline = null;
    ui.toast("已清空全部数据", "success");
  } catch (e) {
    ui.toast(`清空失败：${errText(e)}`, "error");
  }
}

const engines: { key: EngineKind; icon: string; hint: string }[] = [
  { key: "lm_studio", icon: "fa-desktop", hint: "默认 http://localhost:1234/v1" },
  { key: "ollama", icon: "fa-server", hint: "默认 http://localhost:11434/v1" },
  { key: "custom", icon: "fa-plug", hint: "任意 OpenAI 兼容端点" },
];

onMounted(() => {
  if (!llm.models.length) llm.fetchModels().catch(() => undefined);
  llm.refreshLoaded().catch(() => undefined);
  llm.fetchGpu().catch(() => undefined);
  if (isTauri) api.getDataDir().then((dir) => (dataDir.value = dir)).catch(() => undefined);
});

const mb = (bytes: number) => (bytes / 1024 / 1024).toFixed(0) + " MiB";
</script>

<template>
  <div class="h-full overflow-y-auto p-6">
    <div class="mx-auto max-w-3xl space-y-6">
      <header>
        <h2 class="text-xl font-semibold text-slate-100"><i class="fa-solid fa-sliders mr-2 text-jade-500" />LLM 引擎设置</h2>
        <p class="mt-1 text-sm text-slate-400">
          选择本地推理引擎（LM Studio 或 Ollama），应用通过 OpenAI 兼容协议通信，强制 JSON 输出。
        </p>
      </header>

      <!-- 引擎选择 -->
      <section class="panel p-5">
        <span class="label">推理引擎</span>
        <div class="grid grid-cols-1 gap-3 sm:grid-cols-3">
          <button
            v-for="e in engines"
            :key="e.key"
            class="flex flex-col items-start gap-1 rounded-lg border px-4 py-3 text-left transition"
            :class="
              llm.config.engine === e.key
                ? 'border-jade-500 bg-jade-500/10'
                : 'border-ink-500/70 hover:border-ink-500 hover:bg-ink-700/40'
            "
            @click="llm.applyEngine(e.key)"
          >
            <span class="flex items-center gap-2 font-medium text-slate-100">
              <i class="fa-solid" :class="e.icon" />{{ ENGINE_PRESETS[e.key].label }}
            </span>
            <span class="text-[11px] text-slate-500">{{ e.hint }}</span>
          </button>
        </div>

        <div class="mt-4 grid gap-4 sm:grid-cols-2">
          <div>
            <label class="label">服务地址 (OpenAI 兼容根路径)</label>
            <input v-model="llm.config.base_url" class="input" placeholder="http://localhost:1234/v1" />
          </div>
          <div>
            <label class="label">API Key（本地引擎通常留空）</label>
            <input v-model="llm.config.api_key" type="password" class="input" placeholder="可留空" />
          </div>
        </div>
      </section>

      <!-- 模型 -->
      <section class="panel p-5">
        <div class="flex items-center justify-between">
          <span class="label !mb-0">模型</span>
          <button class="btn-ghost !py-1.5" :disabled="llm.loading" @click="llm.fetchModels()">
            <i class="fa-solid fa-rotate" :class="{ 'fa-spin': llm.loading }" />探测模型
          </button>
        </div>

        <div class="mt-3 flex flex-wrap gap-2">
          <span class="text-[11px] text-slate-500">快速选择：</span>
          <button
            v-for="chip in MODEL_CHIPS"
            :key="chip"
            class="chip"
            :class="{ 'chip-active': llm.config.model.includes(chip) }"
            @click="llm.config.model = chip"
          >
            {{ chip }}
          </button>
        </div>

        <div class="mt-3">
          <select v-if="llm.models.length" v-model="llm.config.model" class="input">
            <option v-for="m in llm.models" :key="m.id" :value="m.id">
              {{ m.id }}{{ llm.loadedModels.includes(m.id) ? "  ✓ 已加载" : "" }}
            </option>
          </select>
          <input v-else v-model="llm.config.model" class="input" placeholder="例如 qwen2.5-1.5b-instruct" />

          <div class="mt-2 flex flex-wrap items-center gap-2">
            <button class="btn-ghost !py-1.5" :disabled="llm.loading" @click="llm.activate()">
              <i class="fa-solid fa-power-off" />应用 / 切换模型
            </button>
            <button class="btn-ghost !py-1.5" :disabled="llm.loading" @click="llm.refreshLoaded()">
              <i class="fa-solid fa-rotate" />刷新已加载
            </button>
            <span v-if="llm.loadedModels.length" class="text-[11px] text-jade-400">
              <i class="fa-solid fa-circle-check mr-1" />已加载：{{ llm.loadedModels.join("、") }}
            </span>
            <span v-else class="text-[11px] text-slate-500">未检测到常驻模型</span>
          </div>

          <p class="mt-2 text-[11px] text-slate-500">
            <i class="fa-solid fa-circle-info mr-1" />
            切换模型时应用会尽力卸载旧模型并预热加载新模型；抽取开始前也会自动确保目标模型已激活。
          </p>
        </div>

        <div v-if="llm.recommendedChips.length" class="mt-3">
          <span class="text-[11px] text-slate-500">检测到符合推荐条件的模型：</span>
          <div class="mt-1 flex flex-wrap gap-2">
            <button
              v-for="m in llm.recommendedChips"
              :key="m.id"
              class="chip"
              :class="{ 'chip-active': llm.config.model === m.id }"
              @click="llm.config.model = m.id"
            >
              <i class="fa-solid fa-star mr-1 text-gold-400" />{{ m.id }}
            </button>
          </div>
        </div>
      </section>

      <!-- 参数 -->
      <section class="panel p-5">
        <span class="label">推理参数</span>
        <div class="grid gap-4 sm:grid-cols-2">
          <div>
            <label class="label">Temperature（默认 0.1）</label>
            <div class="flex items-center gap-3">
              <input v-model.number="llm.config.temperature" type="range" min="0" max="1" step="0.05" class="flex-1 accent-jade-500" />
              <span class="w-10 text-right text-sm text-slate-300">{{ llm.config.temperature.toFixed(2) }}</span>
            </div>
          </div>
          <div>
            <label class="label">最大输出 Token</label>
            <input v-model.number="llm.config.max_tokens" type="number" min="256" step="128" class="input" />
          </div>
          <div>
            <label class="label">请求超时（秒）</label>
            <input v-model.number="llm.config.timeout_secs" type="number" min="10" step="10" class="input" />
          </div>
          <div>
            <label class="label">JSON 修复重试次数（默认 3）</label>
            <input v-model.number="llm.config.max_retries" type="number" min="0" max="8" class="input" />
          </div>
        </div>
      </section>

      <div class="flex flex-wrap items-center gap-3">
        <button class="btn-primary" @click="llm.persist()"><i class="fa-solid fa-floppy-disk" />保存配置</button>
        <button class="btn-ghost" :disabled="llm.loading" @click="llm.test()"><i class="fa-solid fa-heart-pulse" />测试连接</button>
        <span v-if="llm.lastError" class="text-xs text-cinnabar-400">{{ llm.lastError }}</span>
        <span v-else-if="llm.connected" class="text-xs text-jade-400">
          <i class="fa-solid fa-circle-check mr-1" />连接正常
        </span>
      </div>

      <section v-if="dataDir" class="panel p-5 text-xs">
        <span class="label">数据存储位置</span>
        <div class="flex items-center gap-2">
          <i class="fa-solid fa-folder-open text-jade-500" />
          <code class="truncate text-slate-300">{{ dataDir }}</code>
        </div>
        <p class="mt-2 leading-relaxed text-slate-500">
          SQLite 数据库（chronospace.sqlite3）与引擎配置（llm_config.json）保存在此用户目录下。原始小说文件不会被复制、移动或修改。
        </p>
        <button class="btn-danger mt-3" @click="clearAllData">
          <i class="fa-solid fa-broom" />清空全部数据
        </button>
      </section>

      <section class="panel p-5 text-xs leading-relaxed text-slate-400">
        <h4 class="mb-2 text-sm font-semibold text-slate-200">本地引擎快速上手</h4>
        <p><strong class="text-jade-400">LM Studio：</strong>加载模型后，在 "Local Server" 标签页启动服务（默认端口 1234），地址填 <code>http://localhost:1234/v1</code>。</p>
        <p class="mt-1"><strong class="text-jade-400">Ollama：</strong>执行 <code>ollama pull qwen2.5:1.5b</code> 与 <code>ollama serve</code>，地址填 <code>http://localhost:11434/v1</code>（应用会自动回退到 <code>/api/tags</code> 列出模型，并通过 <code>/api/ps</code> 显示已加载模型）。</p>
        <p class="mt-1"><strong class="text-jade-400">思考模式：</strong>应用默认发送 <code>reasoning_effort: "none"</code> 关闭思考模型的长推理，避免挤占 JSON 输出；引擎不支持该字段时会自动降级重试。</p>
      </section>

      <section class="panel p-5 text-xs leading-relaxed text-slate-400">
        <h4 class="mb-2 text-sm font-semibold text-slate-200"><i class="fa-solid fa-microchip mr-1 text-gold-400" />多 GPU / 显卡选择</h4>

        <div v-if="llm.gpu && llm.gpu.gpus.length" class="mb-3 space-y-1 rounded-lg bg-ink-900/60 p-3">
          <div v-for="g in llm.gpu.gpus" :key="g.name" class="flex items-center justify-between">
            <span class="text-slate-300"><i class="fa-solid fa-gpu mr-1 text-jade-500" />{{ g.name }}</span>
            <span class="text-slate-400">{{ mb(g.used_bytes) }} / {{ mb(g.total_bytes) }}</span>
          </div>
          <div v-if="llm.gpu.model_vram_bytes" class="pt-1 text-slate-400">
            当前模型显存占用：<span class="text-jade-400">{{ mb(llm.gpu.model_vram_bytes) }}</span>
            · 建议并发：<span class="text-gold-400">{{ llm.gpu.suggested_concurrency }} 路</span>
          </div>
        </div>
        <p v-else-if="llm.gpu" class="mb-2 text-slate-500">未检测到 NVIDIA 显卡（显存探测仅支持 nvidia-smi）。</p>

        <p>模型的加载与显卡分配由<strong>推理引擎</strong>决定，应用无法直接指定 GPU。请按引擎方式配置：</p>
        <p class="mt-1"><strong class="text-jade-400">LM Studio：</strong>在每个模型的「加载设置 / GPU Offload」中选择目标 GPU；或在启动 LM Studio 前设置 <code>CUDA_VISIBLE_DEVICES=0</code>（仅暴露快速卡）。</p>
        <p class="mt-1"><strong class="text-jade-400">Ollama：</strong>关闭服务后设置 <code>CUDA_VISIBLE_DEVICES</code>（NVIDIA）/ <code>ROCR_VISIBLE_DEVICES</code>（AMD）后重启 <code>ollama serve</code>，使模型只落在指定 GPU。</p>
        <p class="mt-1">调整可见设备后，回到本页点击「应用 / 切换模型」重新加载即可。切换模型时，应用会请求卸载旧模型以释放显存。</p>
        <p class="mt-2 border-t border-ink-600/60 pt-2">
          <strong class="text-gold-400">并行抽取：</strong>在「图书馆」抽取面板可将并发设为 1–4 路。应用会并行发起请求，但需引擎支持——<strong>Ollama</strong> 需以 <code>OLLAMA_NUM_PARALLEL&gt;=2</code> 启动；<strong>LM Studio</strong> 需在服务设置中开启并发/批处理。当模型显存占用低于单卡 80% 时，应用会给出建议并发数。
        </p>
      </section>
    </div>
  </div>
</template>
