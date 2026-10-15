# 变更日志

本项目遵循 [语义化版本](https://semver.org/lang/zh-CN/) 与 [Conventional Commits](https://www.conventionalcommits.org/zh-hans/)。
每个有意义的功能实现或修复均递增 PATCH 版本。

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
