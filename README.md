# 时空编年史 · ChronoSpace Reader

> 当前版本：**v0.1.2**

**ChronoSpace Reader（时空编年史）** 是一款跨平台桌面应用，使用 **Tauri 2.0（Rust）+ Vue 3（Vite / Pinia / TypeScript / TailwindCSS）** 构建。
它把 EPUB / MOBI / AZW3 电子书**在内存中**解析分章，借助**本地大模型**（LM Studio 或 Ollama，OpenAI 兼容 API）抽取**时间、空间、人物、物品/功法与因果**结构，持久化到本地 SQLite，并以**交互式时空图谱**呈现，一键导出可部署到 GitHub Pages 的**零依赖单页 HTML**。

全部处理均在本地完成，原文只驻留内存，不上传、不落盘。

---

## ✨ 功能特性

- **内存电子书解析**：EPUB（ZIP + OPF spine 解析）、MOBI / AZW3（PalmDB + PalmDOC 解压）全流程内存处理，流式分块避免磁盘膨胀。
- **本地大模型抽取**：
  - 双引擎支持：**LM Studio**（`http://localhost:1234/v1`）与 **Ollama**（`http://localhost:11434/v1`），亦可接入任意 OpenAI 兼容端点。
  - 自动探测模型列表 `/v1/models`（Ollama 回退 `/api/tags`），内置 `qwen2.5-0.5b / 1.5b / 3b`、`gemma-2-2b` 等快速选择芯片。
  - 强制 JSON 输出（`response_format: { type: "json_object" }`），温度默认 `0.1`。
- **叙事视角归一化**：同时支持第一/第三人称，自动将「我」解析为主角规范名或 `PROTAGONIST_FIRST_PERSON`。
- **批量抽取运行器**：暂停 / 继续 / 取消、章节进度条、**逐章 SQLite 事务检查点**（可断点续跑）。
- **动态时空画布**：
  - 拓扑空间图（Cytoscape.js），按 `world`（人界 / 灵界…）嵌套分组显示 `site` 节点。
  - 双轴时间滑块（章节轴 + 虚拟故事时间线），拖动时主角令牌沿节点动画迁移。
  - 物品/功法谱系抽屉：点击「掌天瓶」高亮其首次出现章节，并追溯其后触发的全部章节。
- **静态 HTML 导出**：内联全部 JSON 数据，生成可直接上传 GitHub Pages 的单页交互式查看器。
- **JSON 修复容错**：模型输出非法 JSON 时自动本地修复，并最多重试 3 次（JSON 修复提示词）。

---

## 🧱 系统架构

```
┌────────────────────────────── 桌面窗口 (Tauri WebView) ──────────────────────────────┐
│  Vue 3 SPA (Pinia + TailwindCSS + Font Awesome + Cytoscape.js)                        │
│   LibraryView   ·   SpacetimeCanvas(SpaceGraph/TimeScrubber/EntityDrawer)   ·  Settings │
└───────────────────────────────────────▲──────────────────────────────────────────────┘
                                         │ Tauri IPC (invoke) / Event (emit)
┌───────────────────────────────────────┴──────────────────────────────────────────────┐
│  Rust 后端 (Tauri 2.0)                                                                │
│   commands ─┬─ parser (epub / mobi / azw3, 全内存)                                     │
│             ├─ pipeline (分块 → LLM → JSON 修复 → 事务持久化 → 进度事件)                │
│             ├─ llm (OpenAI 兼容客户端: LM Studio / Ollama)                             │
│             └─ db (rusqlite, 关系模型 + 迁移)                                          │
│   内存缓存: ParsedBook（原文） · JobControl（暂停/取消）                                │
└───────────────────────────────────────┬──────────────────────────────────────────────┘
                                         │ HTTP (localhost)
                          ┌──────────────┴──────────────┐
                          │  LM Studio  /  Ollama 服务    │
                          └──────────────────────────────┘
```

更完整的数据流、状态机与协议契约见 [`design.md`](./design.md)。

---

## 🚀 快速开始

### 1. 系统依赖（Linux Mint / Ubuntu）

```bash
sudo apt update
sudo apt install -y \
  build-essential curl wget file pkg-config \
  libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
  librsvg2-dev libxdo-dev libssl-dev
```

> 需 Rust ≥ 1.77（`rustup` 安装）与 Node.js ≥ 18。

### 2. 安装项目依赖

```bash
npm install
python3 scripts/gen-icons.py   # 生成占位图标（生产环境请替换为设计稿）
```

### 3. 启动本地大模型

**LM Studio**：加载模型 → `Local Server` 标签页 → `Start Server`（默认 `http://localhost:1234/v1`）。

**Ollama**：

```bash
ollama pull qwen2.5:1.5b
ollama serve                    # 默认 http://localhost:11434/v1
```

### 4. 运行开发版

```bash
npm run tauri:dev
```

在应用内进入 **引擎设置**：

1. 选择引擎（LM Studio / Ollama / 自定义）；
2. 确认服务地址，点击 **探测模型**；
3. 从下拉框或快速芯片中选择模型，`Temperature` 保持 `0.1`；
4. **保存配置** → **测试连接**。

### 5. 使用流程

1. 将 EPUB / MOBI / AZW3 拖入窗口（或点击「选择文件」）。
2. 在「图书馆」选中书籍，点击 **开始抽取**（可随时暂停/继续/取消）。
3. 切换到 **时空画布**：拖动滑块浏览章节轴与虚拟时间线，点击右侧人物/物品查看谱系。
4. 点击书籍卡片 **导出**，生成可交互单页 HTML。

---

## 🛠 构建与发布

```bash
# 仅前端构建
npm run build

# 类型检查
npm run typecheck

# 打包桌面应用（AppImage / deb / rpm 等）
npm run tauri:build
```

产物位于 `src-tauri/target/release/bundle/`。

---

## 📁 目录结构

```
novel_vis/
├── index.html                 # Vite 入口
├── package.json               # 前端依赖与脚本（版本源）
├── src/                       # Vue 3 前端
│   ├── api/tauri.ts           # IPC 封装
│   ├── stores/                # Pinia：ui / library / llm / extraction
│   ├── components/            # LibraryView / SpacetimeCanvas / SpaceGraph / ...
│   ├── exporter/staticHtml.ts # 单页 HTML 导出器
│   └── types/schema.ts        # 与 Rust 对齐的类型契约
├── src-tauri/                 # Rust 后端
│   ├── Cargo.toml             # 依赖与版本（版本源）
│   ├── tauri.conf.json        # 应用配置（版本源）
│   └── src/
│       ├── commands.rs        # Tauri 命令
│       ├── parser/            # epub / mobi / util（全内存解析）
│       ├── pipeline.rs        # 抽取管线 + JSON 修复 + 进度事件
│       ├── llm.rs             # LM Studio / Ollama 客户端
│       ├── db.rs              # SQLite 模式与查询
│       ├── models.rs          # 数据结构与抽取 schema
│       └── state.rs           # 内存缓存 + 任务控制
├── scripts/
│   ├── bump-version.mjs       # 语义化版本递增
│   ├── docs-check.mjs         # 文档-代码一致性校验
│   └── gen-icons.py           # 占位图标生成
├── README.md                  # 本文件
└── design.md                  # 架构蓝图
```

---

## 🔢 版本与提交规范（重要）

本项目在 `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` **三处**维护同一语义化版本，`README.md` 与 `design.md` 必须与代码保持同步。

### 递增版本（每个功能/修复都递增 PATCH）

```bash
npm run bump:patch    # 0.1.0 -> 0.1.1
npm run bump:minor    # 0.1.1 -> 0.2.0
npm run bump:major    # 0.2.0 -> 1.0.0
node scripts/bump-version.mjs 0.3.2   # 指定精确版本
```

`bump-version.mjs` 会同步更新三处版本源，并替换文档中的 `vX.Y.Z` 标记。

### 文档一致性校验

```bash
npm run docs:check    # 校验版本一致且文档包含当前版本标记（CI 可用）
```

### Conventional Commits 提交约定

每一次有意义的实现或修复都应递增 PATCH 版本，并使用如下格式提交：

```bash
git commit -m "feat(core): implement epub parser in memory and sqlite schema v0.1.1"
git commit -m "feat(llm): add lm studio open-ai compatible client with retry v0.1.2"
git commit -m "feat(export): generate standalone single-page html bundle v0.1.3"
git commit -m "docs: update README.md and design.md for spatiotemporal pipeline v0.1.4"
```

推荐工作流：

```bash
npm run bump:patch                 # 1) 递增 PATCH
# 2) 同步更新 README.md / design.md 的变更日志
npm run docs:check                 # 3) 校验文档同步
git add -A && git commit -m "feat(core): ... v0.1.1"
```

---

## ⚠️ 已知限制

- **DRM / HUFF-CDIC**：加密电子书与 HUFF/CDIC 压缩的 MOBI 暂不支持（会给出明确错误）。
- **原文缓存会话级**：为避免磁盘膨胀，原文仅驻留内存；应用重启后需重新导入再抽取（已抽取的结构化数据持久保存）。
- **长章节分块**：超长章节按约 2k token 分块，逐块抽取后各自落库为独立时空节点。

---

## 📝 变更日志

各版本变更记录见 [`CHANGELOG.md`](./CHANGELOG.md)。

---

## 📄 许可证

本项目仅用于个人学习与研究。请遵守所解析电子书的版权与授权条款。
