# 变更日志

本项目遵循 [语义化版本](https://semver.org/lang/zh-CN/) 与 [Conventional Commits](https://www.conventionalcommits.org/zh-hans/)。
每个有意义的功能实现或修复均递增 PATCH 版本。

## [0.1.20] - refactor(canvas)

- 清理时空画布重复面板：移除 `StoryTimeline`（其章节刻度、跳跃/倒叙标记、点击跳转已由时空矩阵与顶部时间轴承担）。
- 将当前章节标题/时间/移动/地点徽标合并进「剧情摘要」面板，并移除 `TimeScrubber` 中重复的章节信息行。

## [0.1.19] - feat(canvas)

- 时空画布重做为「时间 × 空间」矩阵：横轴为章节、每行一个空间，人物首现圆点 + 跨行/跨列轨迹；`chunk_chars=0` 支持整章输入（不切块），前端新增「整章」开关。
- 画布叠加人物关系：同章共现人物以虚线连接（可开关）；选中人物后高亮其关系网与行踪。
- 点击行查看地点元数据、点击轨迹查看人物谱系；顶部时间轴与左侧空间标签固定显示。

## [0.1.18] - feat(extract)

- 并行抽取：`ExtractionRequest.concurrency`（1–4）；`run_extraction` 以 `JoinSet` 分批并发处理章节，DB 写入串行化。
- 新增 `gpu_status`：通过 nvidia-smi 读取显存、Ollama `/api/ps` 读取模型显存；当模型占用低于单卡 80% 时给出建议并发数。
- 抽取面板新增「并发」选择与建议提示；引擎设置新增显存信息与并行配置说明（Ollama `OLLAMA_NUM_PARALLEL`、LM Studio 并发设置）。

## [0.1.17] - fix(extract)

- 重新抽取（取消勾选「跳过已完成」）前先清除该书旧抽取数据（`clear_book_extraction`：节点/实体/事件/章节摘要），避免重复或错乱；书籍与章节记录保留。
- 前端开始前弹出确认警告，并在复选框处提示「将清空重抽」。

## [0.1.16] - feat(llm)

- 模型切换：新增 `activate_model`（卸载旧模型 + 预热加载新模型）与 `list_loaded_models`（Ollama `/api/ps`、LM Studio `/api/v0/models`）；`start_extraction` 前自动激活目标模型，`AppState.active_model` 记录当前激活模型。
- 设置页显示已加载模型、新增「应用 / 切换模型」「刷新已加载」按钮，并补充**多 GPU / 显卡选择**说明（显卡分配属引擎侧设置）。

## [0.1.15] - feat(ui)

- 新增「人物关系」标签页 `RelationGraph.vue`：人物为节点（按出场次数缩放）、同章共现为连线（线宽=共现次数），点击高亮关系网与出场章节。
- 新增「空间物品」标签页 `EntityCards.vue`：地点卡片（世界/区域、章节数、在场人物、获得物品）与物品/功法卡片（类别、首现、获得地点、次数），支持搜索。
- 修复时空图箭头/连线需点击后才显示：初始化即构建元素，并在容器尺寸确定后（`requestAnimationFrame` + `ResizeObserver`）自动适配；STAY 边提高可见度（虚线）。
- 强化 JSON 输出约束：提示词开篇强制「只输出一个 JSON 对象」，附输出前自检清单与结尾重申；默认分块 3000 → 2000。

## [0.1.14] - fix(core)

- 源文件可重定位：`books` 新增 `source_path` / `content_sig`（SHA-256）；抽取开始时若原文不在内存，`ensure_book_loaded` 会按记录路径重新读取并校验签名（旧库自动增量迁移补列）。
- 新增 `SOURCE_MISSING` / `SOURCE_MISMATCH` 错误码与 `set_book_source` / `get_book_source` 命令；前端在缺失/不匹配时引导重新选择文件，内容不一致需确认强制关联。
- 修复错误提示显示 `[object Object]`：新增 `errText`/`errCode`，将 `{code,message}` 错误渲染为可读文本。

## [0.1.13] - fix(ui)

- 提升空间图可读性：地点卡片改为高对比深色渐变填充 + 亮色描边 + 浅色加粗文字（带描边），世界面板加实线边框与金色标题；节点加入投影。
- 加大横向/纵向间距（CELL 200×124，世界间隔 220），STAY 边淡化（opacity 0.22）凸显移动边；图例配色同步。

## [0.1.12] - fix(llm)

- JSON 修复增强：补全缺失逗号（`expected , or }`）、补齐被截断的闭合括号；解析前去除 BOM。
- LLM 客户端支持 `reasoning_content` / `reasoning` 回退，修复推理模型 `content` 为空导致的「未找到 JSON 对象」。
- 提示词新增「输出精简完整」要求并提高默认 `max_tokens`（2048 → 3072），降低截断概率。
- 修复图谱崩溃：地点节点 id 缺 `site::` 前缀导致节点未创建、边引用缺失（`Can not create edge ... nonexistent source`）；边创建前校验节点存在，`cy.animate` 拒绝被吞掉，渲染异常不再冒泡为未处理 Promise 拒绝。

## [0.1.11] - fix(llm)

- 提示词题材无关化：移除所有示例专有名词（避免小模型照搬“韩立/掌天瓶/人界”等造成污染），明确 `world` 可为现实语境，不强制“界”类术语。
- 抗幻觉实体校验：`ground_entries` 按当前分块正文过滤未出现的人物/物品（`PROTAGONIST_FIRST_PERSON` 与单字名豁免），过滤数经日志上报。
- 新增 `clear_all_data` 命令与设置页「清空全部数据」（带确认），便于清除被污染数据。

## [0.1.10] - fix(llm)

- 修复抽取失败：JSON 修复新增「字符串内裸控制字符转义」（解决 `control character found while parsing a string`），并将 JSON 提取改为平衡括号扫描（容忍数组包裹与尾随文本）。
- 重要操作确认：新增全局确认框（`ui.askConfirm` + `ConfirmDialog.vue`），删除书籍、取消抽取均需确认。
- 空间图改为网格卡片式布局并美化（圆角卡片、按出现次数加强边框、点阵背景）；点击地点弹出元数据面板（出现章节、人物、物品/功法、时间标记）。

## [0.1.9] - feat(ui)

- 修复多本切换抽取：抽取状态改为**按书籍独立的任务**（前端 `jobs` 映射 + 后端按 `book_id` 注册 `JobControl`），进度/日志事件携带 `book_id` 路由；可暂停一本并同时抽取另一本。
- 重做空间图：确定性布局（世界分行、地点环状），移动类型着色的边、按出现次数缩放的地点节点、缩放/适应窗口控件。
- 新增可视化时间线 `StoryTimeline.vue`：章节刻度、时间跳跃/倒叙标记与天数、实体事件高亮、点击跳转。

## [0.1.8] - docs

- 新增**强制提交规则**（`design.md` §11.1、README）：任何有意义的变更完成后必须立即提交并递增 PATCH，提交后保持工作区干净；仅明确要求时合并提交。
- 本版本一次性提交了 v0.1.3 → v0.1.8 的累积变更（用户明确要求）。

## [0.1.7] - feat(import)

- 合订本（一文件多书）探测：解析 EPUB3 `nav.xhtml` 与 EPUB2 `toc.ncx` 顶级分组，映射为章节区间候选。
- 新增 `inspect_book` 命令（只读探查）与 `MultiBookDialog.vue`；导入前由用户确认「作为多本导入 / 作为单本导入」。
- `ingest_book` 支持 `splitIntoBooks`，新增 `split_book`（按候选拆分并重排章节索引，未覆盖章节归入「书名 · 其他」）。
- 新增 NCX/nav 探测与拆分的单元测试。

## [0.1.6] - feat(i18n)

- 新增中文友好解码 `parser::encoding`：支持 UTF-8 / GBK / GB18030 / Big5，含 BOM 检测、XML 编码声明嗅探，以及「GBK 字节被误标为 windows-1252」的启发式纠正。
- EPUB 章节按 XML `encoding="..."` 声明解码；MOBI `textEncoding` 数值映射为编码名，1252/未知交由启发式判定。
- LLM 系统提示词明确要求自然语言字段输出简体中文，仅 JSON 键名与枚举保持英文，专有名词保留原写法。
- 章节标题推测扩展中文标记（番外/终章/外传/尾声/引子/后记）。
- 新增编码相关单元测试。

## [0.1.5] - fix(parser)

- 修复 `extract_html_title` 在按固定字节截断时遇到多字节字符（中文）触发的 panic（`byte index ... is not a char boundary`）；改为按 UTF-8 字符边界截断。
- 新增回归测试：长中文标题不 panic、标题抽取与字符边界工具函数。

## [0.1.4] - test

- 新增 Rust 测试套件（26 个单元测试 + 1 个集成测试）：解析、JSON 修复/分块、SQLite 持久化、数据目录解析。
- 修复 EPUB 解析缺陷：OPF 中自闭合的 `<item/>` / `<itemref/>`（`Event::Empty`）此前被忽略，导致清单/书脊为空；由新增测试捕获。
- `package.json` 新增 `test` / `test:rust` 脚本。

## [0.1.3] - fix(storage)

- 数据库与引擎配置改存用户配置目录 `local/<APPNAME>`（Linux `~/.config/local/chronospace-reader/`），不再使用 Tauri app-data 目录；新增 `paths.rs` 统一解析，支持 `CHRONOSPACE_DATA_DIR` 覆盖。
- 新增 `get_data_dir` 命令，引擎设置页展示数据存储位置。

## [0.1.2] - fix(tooling)

- `bump-version.mjs` 现同步更新 `src-tauri/Cargo.lock` 中本包版本，避免 cargo 二次改写产生游离改动。

## [0.1.1] - feat(ui)

- 应用头部显示当前版本号（由 `vite.config.ts` 从 `package.json` 注入 `__APP_VERSION__`）。
- 新增本变更日志，并纳入文档同步流程。

## [0.1.0] - 初始版本

- Tauri 2.0 + Vue 3 + Vite + Pinia + TypeScript + TailwindCSS + Font Awesome 工程搭建。
- 全内存 EPUB（ZIP/OPF spine）与 MOBI/AZW3（PalmDB + PalmDOC 解压）解析。
- SQLite 关系模式：`books` / `chapters` / `spatiotemporal_nodes` / `entities` / `entity_events`。
- LM Studio 与 Ollama 的 OpenAI 兼容客户端，强制 JSON 输出并带 JSON 修复重试。
- 批量抽取管线：暂停 / 继续 / 取消、章节进度、逐章事务检查点。
- Cytoscape 拓扑空间图、章节轴 + 虚拟时间线双滑块、人物/物品谱系抽屉。
- 零依赖单页 HTML 导出器（GitHub Pages 友好）。
- `bump-version.mjs` 与 `docs-check.mjs` 版本/文档同步工具。
