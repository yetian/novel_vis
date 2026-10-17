/// 全书范围的重要度：人物、空间、物品/功法。
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

function normalizedExplicit(explicit?: number | null): number | null {
  return explicit != null && explicit >= 1 && explicit <= 5 ? Math.round(explicit) : null;
}

function clampImportance(raw: number): number {
  return Math.max(1, Math.min(5, Math.ceil(raw)));
}

export interface SpaceImportanceFactors {
  explicit?: number | null;
  coverage: number;
  volume: number;
  spanRatio: number;
  richness: number;
}

/// 空间重要度：模型判断 + 覆盖章节 + 事件频率 + 跨度 + 出场人物丰富度。
export function computeSpaceImportance({
  explicit,
  coverage,
  volume,
  spanRatio,
  richness,
}: SpaceImportanceFactors): number {
  const modelScore = normalizedExplicit(explicit);
  const raw =
    modelScore != null
      ? 0.45 * modelScore +
        5 * (0.15 * coverage + 0.15 * volume + 0.1 * spanRatio + 0.15 * richness)
      : 5 * (0.3 * coverage + 0.25 * volume + 0.2 * spanRatio + 0.25 * richness);
  return clampImportance(raw);
}

export const SPACE_IMPORTANCE_LABELS: Record<number, string> = {
  5: "核心地点",
  4: "主要地点",
  3: "重要地点",
  2: "次要地点",
  1: "过场地点",
};

export function spaceImportanceLabel(n: number): string {
  return SPACE_IMPORTANCE_LABELS[n] ?? "—";
}

export interface ThingImportanceFactors {
  explicit?: number | null;
  coverage: number;
  volume: number;
  spanRatio: number;
}

/// 物品/功法重要度：模型判断 + 覆盖章节 + 出现频率 + 跨度。
export function computeThingImportance({
  explicit,
  coverage,
  volume,
  spanRatio,
}: ThingImportanceFactors): number {
  const modelScore = normalizedExplicit(explicit);
  const raw =
    modelScore != null
      ? 0.5 * modelScore + 5 * (0.2 * coverage + 0.15 * volume + 0.15 * spanRatio)
      : 5 * (0.35 * coverage + 0.35 * volume + 0.3 * spanRatio);
  return clampImportance(raw);
}

export const THING_IMPORTANCE_LABELS: Record<number, string> = {
  5: "核心物品",
  4: "关键物品",
  3: "重要物品",
  2: "次要物品",
  1: "过场物品",
};

export function thingImportanceLabel(n: number): string {
  return THING_IMPORTANCE_LABELS[n] ?? "—";
}
