/// 全书范围的人物重要度：模型判断 + 章节覆盖率 + 出场频率 + 时间跨度。
export interface ImportanceFactors {
  explicit?: number | null;
  coverage: number;
  volume: number;
  spanRatio: number;
  role?: string | null;
}

export function computeImportance({
  explicit,
  coverage,
  volume,
  spanRatio,
  role,
}: ImportanceFactors): number {
  const modelScore =
    explicit != null && explicit >= 1 && explicit <= 5 ? Math.round(explicit) : null;
  let raw: number;
  if (modelScore != null) {
    raw = 0.5 * modelScore + 5 * (0.2 * coverage + 0.15 * volume + 0.15 * spanRatio);
  } else {
    raw = 5 * (0.35 * coverage + 0.35 * volume + 0.3 * spanRatio);
  }
  if (/主角|男主|女主|protagonist/i.test(role || "")) raw += 0.5;
  else if (/反派|主要|核心|boss/i.test(role || "")) raw += 0.25;
  return Math.max(1, Math.min(5, Math.ceil(raw)));
}

export const IMPORTANCE_LABELS: Record<number, string> = {
  5: "核心主角",
  4: "主要人物",
  3: "重要配角",
  2: "次要人物",
  1: "路人",
};

export function importanceLabel(n: number): string {
  return IMPORTANCE_LABELS[n] ?? "—";
}
