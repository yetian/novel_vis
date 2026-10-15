#!/usr/bin/env node
/**
 * 语义化版本递增脚本（PATCH 优先）。
 *
 * 用法：
 *   node scripts/bump-version.mjs patch    # 0.1.0 -> 0.1.1
 *   node scripts/bump-version.mjs minor    # 0.1.1 -> 0.2.0
 *   node scripts/bump-version.mjs major    # 0.2.0 -> 1.0.0
 *   node scripts/bump-version.mjs 0.3.2    # 指定精确版本
 *
 * 会同步更新：package.json / src-tauri/Cargo.toml / src-tauri/tauri.conf.json
 * 以及 README.md、design.md 中的版本标记，确保文档与代码严格一致。
 */
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const pkgPath = join(root, "package.json");
const cargoPath = join(root, "src-tauri", "Cargo.toml");
const tauriPath = join(root, "src-tauri", "tauri.conf.json");
const docPaths = [join(root, "README.md"), join(root, "design.md")];

function bump(current, kind) {
  if (/^\d+\.\d+\.\d+$/.test(kind)) return kind;
  const [maj, min, pat] = current.split(".").map((n) => parseInt(n, 10));
  switch (kind) {
    case "major":
      return `${maj + 1}.0.0`;
    case "minor":
      return `${maj}.${min + 1}.0`;
    case "patch":
      return `${maj}.${min}.${pat + 1}`;
    default:
      throw new Error(`未知的版本类型: ${kind}（可用 patch/minor/major 或精确版本号）`);
  }
}

function main() {
  const kind = process.argv[2] ?? "patch";
  const pkg = JSON.parse(readFileSync(pkgPath, "utf8"));
  const oldVersion = pkg.version;
  const newVersion = bump(oldVersion, kind);

  pkg.version = newVersion;
  writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + "\n");

  const cargo = readFileSync(cargoPath, "utf8").replace(
    /(^\[package\][\s\S]*?^version\s*=\s*")[^"]+(")/m,
    `$1${newVersion}$2`,
  );
  writeFileSync(cargoPath, cargo);

  const tauri = JSON.parse(readFileSync(tauriPath, "utf8"));
  tauri.version = newVersion;
  writeFileSync(tauriPath, JSON.stringify(tauri, null, 2) + "\n");

  // 同步文档中的版本标记 `vX.Y.Z`
  for (const docPath of docPaths) {
    try {
      const doc = readFileSync(docPath, "utf8").replaceAll(`v${oldVersion}`, `v${newVersion}`);
      writeFileSync(docPath, doc);
    } catch {
      // 文档尚不存在时跳过
    }
  }

  console.log(`版本已更新: ${oldVersion} -> ${newVersion}`);
  console.log("请同步更新 README.md / design.md 的变更日志，然后提交：");
  console.log(`  git commit -m "chore(release): bump version to v${newVersion}"`);
}

main();
