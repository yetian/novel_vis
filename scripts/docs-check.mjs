#!/usr/bin/env node
/**
 * 文档-代码一致性校验。
 *
 * 校验 package.json、src-tauri/Cargo.toml、src-tauri/tauri.conf.json 的版本一致，
 * 且 README.md、design.md 中包含 `v<version>` 标记。CI 或提交前可运行。
 *
 *   node scripts/docs-check.mjs
 */
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (p) => readFileSync(join(root, p), "utf8");

const pkg = JSON.parse(read("package.json"));
const version = pkg.version;

const cargoVersion = read("src-tauri/Cargo.toml").match(
  /^\[package\][\s\S]*?^version\s*=\s*"([^"]+)"/m,
)?.[1];

const tauriVersion = JSON.parse(read("src-tauri/tauri.conf.json")).version;

const errors = [];
if (cargoVersion !== version) {
  errors.push(`Cargo.toml 版本 (${cargoVersion}) 与 package.json (${version}) 不一致`);
}
if (tauriVersion !== version) {
  errors.push(`tauri.conf.json 版本 (${tauriVersion}) 与 package.json (${version}) 不一致`);
}

for (const doc of ["README.md", "design.md"]) {
  const content = read(doc);
  if (!content.includes(`v${version}`)) {
    errors.push(`${doc} 未包含当前版本标记 v${version}`);
  }
}

if (errors.length) {
  console.error("文档同步校验失败：");
  for (const e of errors) console.error(`  - ${e}`);
  console.error("\n请运行 `npm run bump:patch` 或手动同步后重试。");
  process.exit(1);
}

console.log(`文档同步校验通过：v${version}（package.json / Cargo.toml / tauri.conf.json / README.md / design.md）`);
