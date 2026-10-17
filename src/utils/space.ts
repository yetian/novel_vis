// 空间字段清洗：早期提示词会注入“现实世界”等泛化标签，
// 这里仅用于展示层清理；新抽取要求 world 必须来自原文。
const GENERIC_WORLDS = new Set([
  "现实世界",
  "现实",
  "真实世界",
  "现代世界",
  "未知世界",
  "未知空间",
  "未标注空间",
  "未标注",
]);

export function cleanWorld(w?: string | null): string {
  const s = (w || "").trim();
  return s && !GENERIC_WORLDS.has(s) ? s : "";
}

export function cleanPlace(s?: string | null): string {
  const v = (s || "").trim();
  return GENERIC_WORLDS.has(v) ? "" : v;
}
