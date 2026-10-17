# 时空编年史 · ChronoSpace Reader

> 当前版本：**v0.1.24**

**ChronoSpace Reader（时空编年史）** 是一款跨平台桌面应用，使用 **Tauri 2.0（Rust）+ Vue 3（Vite / Pinia / TypeScript / TailwindCSS）** 构建。
它把 EPUB / MOBI / AZW3 电子书**在内存中**解析分章，借助**本地大模型**（LM Studio 或 Ollama，OpenAI 兼容 API）抽取**时间、空间、人物、物品/功法与因果**结构，持久化到本地 SQLite，并以**交互式时空图谱**呈现，一键导出可部署到 GitHub Pages 的**零依赖单页 HTML**。

全部处理均在本地完成，原文只驻留内存，不上传、不落盘。

---

## ✨ 功能特性

- **内存电子书解析**：EPUB（ZIP + OPF spine 解析）、MOBI / AZW3（PalmDB + PalmDOC 解压）全流程内存处理，流式分块避免磁盘膨胀。
- **合订本拆分**：解析 EPUB 目录（NCX / EPUB3 nav）顶级分组，自动探测「一个文件包含多本书」；导入前弹出确认对话框，由用户选择**作为多本分别导入**或**作为单本导入**（候选未覆盖的章节归入《书名 · 其他》）。
- **中文友好**：面向中文小说设计。解析层支持 **UTF-8 / GBK / GB18030 / Big5**（含 BOM 检测与「把 GBK 误标为 windows-1252」的启发式纠正）；LLM 提示词要求所有自然语言字段输出**简体中文**，仅 JSON 键名与枚举值（如 `CONTINUOUS`、`ITEM`）保持英文；界面与导出页使用中文无衬线字体栈（Noto Sans SC / 苹方 / 微软雅黑）。
- **本地大模型抽取**：
  - 双引擎支持：**LM Studio**（`http://localhost:1234/v1`）与 **Ollama**（`http://localhost:11434/v1`），亦可接入任意 OpenAI 兼容端点。
  - 自动探测模型列表 `/v1/models`（Ollama 回退 `/api/tags`），内置 `qwen2.5-0.5b / 1.5b / 3b`、`gemma-2-2b` 等快速选择芯片；显示引擎中**已加载**的模型。
  - **模型切换**：切换模型时尽力卸载旧模型并预热加载新模型（Ollama 用 `keep_alive:0` 卸载 + `keep_alive:10m` 预热；LM Studio 用极小请求触发即时加载）；抽取前自动确保目标模型已激活。显卡分配属引擎侧设置（见「引擎设置」的多 GPU 说明）。
  - 强制 JSON 输出（`response_format: { type: "json_object" }`），温度默认 `0.1`。
- **叙事视角归一化**：同时支持第一/第三人称，自动将「我」解析为主角规范名或 `PROTAGONIST_FIRST_PERSON`。
- **题材无关 & 抗幻觉**：提示词不预设题材（现实/都市/历史/科幻/武侠/玄幻皆可）、不带任何示例人名地名；要求**尽可能列出所有具名人物**（含次要/仅被提及者），空间与世界名称**只取原文内容**，不注入“现实世界”等泛化标签；人物、空间与物品/功法均给出 1–5 重要度；模型输出后按正文**校验实体**，剔除未在原文出现的条目。
- **并行抽取**：抽取面板可将并发设为 **1–4 路**；应用并行发起请求（需引擎支持并行/批处理：Ollama 需 `OLLAMA_NUM_PARALLEL≥2`，LM Studio 需开启并发预测）。应用会探测显存（`nvidia-smi` + Ollama `/api/ps`），当模型占用低于单卡 80% 时给出建议并发数。
- **批量抽取运行器**：暂停 / 继续 / 取消、章节进度条、**逐章 SQLite 事务检查点**（可断点续跑）。**支持多本并发**：每个任务按书籍独立跟踪，可暂停一本并同时抽取另一本。关闭「跳过已完成」将重新处理整本书——**会清除该书旧抽取数据并弹出确认**，避免数据重复或错乱。
- **章节总览**：**点击书籍即进入总览**（也可从书库卡片进入）。顶部展示**全书总览**（所有章节抽取完成后自动生成，可手动重新生成）；按章节列出剧情摘要，并汇总章节/人物/物品/地点数量；每条摘要附**本章主要人物**、**首次登场人物**、首现时空与时间跳跃/倒叙，支持全文搜索。
- **章节标题修复**：解析 EPUB 时优先读取正文 h1/h2/h3，而不是经常写着书名的 `<title>`；旧数据在重新加载源文件时自动修正，界面也会避免把书名当作章节名显示。
- **动态时空画布（时间 × 空间矩阵）**：
  - **横轴为时间（章节），每一行是一个空间/地点**；人物首次出现处标注圆点，轨迹连线跨行、跨列延伸，直观展示「谁在何时去了何地」。人物使用 **7 色高对比调色板**区分。
  - **人物关系叠加**：同一章节共同出场的人物以虚线连接（可开关）；点选人物轨迹后高亮其关系网与完整行踪。
  - 时间跳跃（▲）/ 倒叙（▼）标记、当前章节游标与当前空间行高亮；行标签显示出现章节数/人物数。**点击行查看地点元数据**（出现章节、人物、物品/功法、时间标记、事件次数），面板内容过多时可独立滚动；点击轨迹或地点内人物查看谱系。
  - 双轴时间滑块（章节轴 + 虚拟故事时间线），并显示当前章节标题、地点与时间类型。
- **人物总览表（独立标签页）**：按**全书范围的重要度**（模型判断 + 覆盖章节 + 出场频率 + 时间跨度，输出 1–5 级）降序列出姓名、简介、首次/最近出场、首现时空、主要活动地，以及与他人的共现章节数与共现区间；**自动排除作者署名**，支持搜索、关系跳转与“仅主要人物”过滤。
- **空间与物品总览表（独立标签页）**：空间与物品/功法均按**全书范围的重要度**（模型判断 + 覆盖章节 + 频率 + 跨度；空间另计人物丰富度，输出 1–5 级）降序排列。空间表展示地点、世界/区域、首现/最近章节、出现章节数与事件数、按出场排序的人物与物品/功法；物品表展示类别、首现章节、获得地点/章节、使用/描述、关联人物、出现次数与章节数。支持搜索与“仅重要”过滤。世界/地区仅展示原文给出的内容。
- **静态 HTML 导出**：内联全部 JSON 数据，生成可直接上传 GitHub Pages 的单页交互式查看器。
- **强化 JSON 输出约束**：系统提示词开篇即强制「只输出一个 JSON 对象、以 `{` 开头以 `}` 结尾」，含输出前自检清单；配合 `response_format: json_object` 与默认 2000 字分块，降低格式错误。若使用高上下文模型，可勾选「整章」直接将整章文本送入模型，不再切块。
- **JSON 修复容错**：模型输出非法 JSON 时自动本地修复——平衡括号提取、去除注释/尾随逗号、**转义字符串内裸控制字符**、**补全缺失逗号**、**补齐被截断的闭合括号**；并支持从推理模型的 `reasoning_content` 字段回退取内容。失败后再用修复提示词重试 ≤3 次。
- **重要操作确认**：删除书籍、取消抽取等破坏性操作均弹出确认对话框。
- **源文件可重定位**：记录源文件路径与内容签名；重启后再次抽取会自动按路径重新加载原文，若文件被移动或替换会提示重新选择（内容不一致时需确认是否强制关联）。

---

## 🧱 系统架构

```
┌────────────────────────────── 桌面窗口 (Tauri WebView) ──────────────────────────────┐
│  Vue 3 SPA (Pinia + TailwindCSS + Font Awesome)                                       │
│   LibraryView   ·   SummaryView   ·   SpacetimeCanvas(SpaceGraph/TimeScrubber)   ·  Settings │
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
3. 点击书籍进入 **总览**，逐章查看摘要；或切换到 **时空画布**，拖动滑块浏览章节轴与虚拟时间线，点击行/轨迹/人物查看详情。
4. 点击书籍卡片 **导出**，生成可交互单页 HTML。

---

## 💾 数据存储位置

数据保存在**用户配置目录**下的 `local/<APPNAME>` 命名空间（`APPNAME = chronospace-reader`）：

| 平台 | 路径 |
| --- | --- |
| Linux / Mint | `~/.config/local/chronospace-reader/` |
| macOS | `~/Library/Application Support/local/chronospace-reader/` |
| Windows | `%APPDATA%\local\chronospace-reader\` |

其中：

- `chronospace.sqlite3` —— 结构化抽取结果（书籍 / 章节 / 时空节点 / 实体 / 事件）。
- `llm_config.json` —— 引擎配置（LM Studio / Ollama / 自定义端点）。

可用环境变量 `CHRONOSPACE_DATA_DIR` 覆盖该目录。**原始小说文件不会被复制、移动或修改**，仅在导入时读入内存；应用重启后需重新导入才能再次抽取。

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

## 🧪 测试

后端（Rust）测试覆盖解析、抽取与持久化的关键逻辑：

```bash
npm run test          # 等价于 npm run test:rust -> cargo test
```

- **单元测试**（`#[cfg(test)]`，共 45 项）：`parser::util`（HTML 清洗/实体解码/哈希/标题推测）、`parser::encoding`（UTF-8 / GBK / BOM / XML 编码嗅探 / 中文可读性打分）、`parser::epub`（内存 ZIP 构造的端到端 EPUB 解析、NCX/EPUB3 nav 合订本探测、UTF-8 边界安全的标题抽取）、`parser::mobi`（PalmDOC 解压）、`parser::mod`（格式探测 + 合订本拆分与再索引）、`pipeline`（JSON 修复/重试/分块/视角归一化）、`db`（模式 + 持久化 + 级联删除 + 幂等）、`paths`（数据目录解析）。
- **集成测试**（`src-tauri/tests/pipeline_db_roundtrip.rs`）：通过公开 API 验证「LLM JSON → SQLite → 时空聚合查询」全链路。

前端目前仅有类型检查（`npm run typecheck`）。

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
# 2) 同步更新 README.md / design.md / CHANGELOG.md
npm run docs:check                 # 3) 校验文档同步
npm run test                       # 4) 运行测试
git add -A && git commit -m "feat(core): ... v0.1.1"
```

> **提交时机（强制）**：**任何有意义的功能、修复或文档更新完成后必须立即提交**，不得让不相关改动长期堆积。提交后工作区应保持干净（`git status` 无改动）。仅当明确要求「一次性提交」时才合并多笔变更。详见 `design.md` §11.1。

---

## ⚠️ 已知限制

- **DRM / HUFF-CDIC**：加密电子书与 HUFF/CDIC 压缩的 MOBI 暂不支持（会给出明确错误）。
- **原文不落盘**：为避免磁盘膨胀，原文仅驻留内存；但会记录**源文件路径与内容签名**，重启后再次抽取会自动按路径重新加载并校验签名（无需重新导入）；若源文件被移动或替换会提示重新定位。若源文件已删除，则需重新选择。
- **长章节分块**：超长章节按约 2k token 分块，逐块抽取后各自落库为独立时空节点。

---

## 📝 变更日志

各版本变更记录见 [`CHANGELOG.md`](./CHANGELOG.md)。

---

## 📄 许可证

本项目仅用于个人学习与研究。请遵守所解析电子书的版权与授权条款。
