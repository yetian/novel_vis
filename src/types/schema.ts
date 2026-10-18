// 与 src-tauri/src/models.rs 严格对应的前端类型定义。

export type EngineKind = "lm_studio" | "ollama" | "custom";

export interface LlmConfig {
  engine: EngineKind;
  base_url: string;
  api_key?: string | null;
  model: string;
  temperature: number;
  max_tokens: number;
  timeout_secs: number;
  max_retries: number;
}

export interface ModelInfo {
  id: string;
  owned_by?: string | null;
}

export interface IngestResult {
  book_id: number;
  title: string;
  author?: string | null;
  format: string;
  total_chapters: number;
}

export interface BookCandidate {
  /** 稳定目录路径，例如 "0" / "0.1" / "0.1.2"。 */
  id: string;
  title: string;
  start_index: number;
  end_index: number;
  chapter_count: number;
  depth: number;
  children: BookCandidate[];
}

export interface BookInspection {
  title: string;
  author?: string | null;
  format: string;
  total_chapters: number;
  suspected_multi_book: boolean;
  signals: string[];
  candidates: BookCandidate[];
}

export interface BookRow {
  id: number;
  title: string;
  author?: string | null;
  format: string;
  total_chapters: number;
  created_at: string;
  extracted_chapters: number;
  source_path?: string | null;
  /** 全书总览；仅当全书抽取完成后生成。 */
  summary?: string | null;
}

export interface SourceCheck {
  matched: boolean;
  message: string;
}

export interface BiographyKey {
  kind: string;
  target_key: string;
}

export interface BiographyRow extends BiographyKey {
  title: string;
  content: string;
  updated_at: string;
}

export interface BiographyProgress {
  book_id: number;
  kind: string;
  target_key: string;
  current: number;
  total: number;
  message: string;
}

export interface DetailEvent {
  id: number;
  chapter_index: number;
  chapter_title: string;
  action: string;
  role?: string | null;
  place?: string | null;
  time_type?: string | null;
  time_offset_days?: number | null;
  detail?: string | null;
}

export interface DetailRoleTrace {
  chapter_index: number;
  chapter_title: string;
  place?: string | null;
  action?: string | null;
  detail?: string | null;
}

export interface DetailRole {
  name: string;
  first_chapter: number;
  first_chapter_title: string;
  first_place?: string | null;
  last_chapter: number;
  count: number;
  level_up: boolean;
  trace: DetailRoleTrace[];
}

export interface DetailRelation {
  name: string;
  self_role?: string | null;
  other_role?: string | null;
  weight: number;
  first_chapter: number;
  last_chapter: number;
}

export interface DetailTarget {
  kind: "character" | "item" | "skill" | "legacy" | "space";
  key: string;
  title: string;
  subtitle?: string;
  icon?: string;
  metadata: { label: string; value: string }[];
  events: DetailEvent[];
  roles?: DetailRole[];
  relations?: DetailRelation[];
}

export interface ChapterRow {
  id: number;
  book_id: number;
  chapter_index: number;
  title: string;
  raw_text_summary?: string | null;
  temporal_type?: string | null;
  time_offset_days?: number | null;
  world?: string | null;
  region?: string | null;
  site?: string | null;
  movement_type?: string | null;
  /** 1–5，本段地点的重要度（旧数据可能为空）。 */
  spatial_importance?: number | null;
}

export interface EntityRow {
  id: number;
  book_id: number;
  entity_type: string;
  name: string;
  first_appeared_chapter_id?: number | null;
  first_appeared_chapter_index?: number | null;
  description?: string | null;
  /** 1–5，模型从原文判断的实体重要度（人物/物品/功法，旧数据可能为空）。 */
  importance?: number | null;
}

export interface EntityEventRow {
  id: number;
  entity_id: number;
  chapter_id: number;
  chapter_index: number;
  action_type: string;
  role?: string | null;
  acquired_site?: string | null;
  impact_detail?: string | null;
}

export interface TimelineBundle {
  book: BookRow;
  chapters: ChapterRow[];
  entities: EntityRow[];
  events: EntityEventRow[];
}

export interface LogEvent {
  book_id: number;
  message: string;
}

export interface SiteSelection {
  world: string;
  region?: string | null;
  site: string;
}

export interface ProgressEvent {
  book_id: number;
  phase: string;
  current: number;
  total: number;
  chapter_index: number;
  chapter_title: string;
  message: string;
  retries: number;
}

export interface ChapterExtractionResult {
  chapter_index: number;
  chapter_title: string;
  ok: boolean;
  retries_used: number;
  repaired: boolean;
  extraction?: SegmentExtraction | null;
  error?: string | null;
}

export interface Temporal {
  type: "CONTINUOUS" | "JUMP_FORWARD" | "FLASHBACK" | "UNCERTAIN";
  raw_marker?: string | null;
  relative_delta_days?: number | null;
  reference_anchor?: string | null;
}

export interface Spatial {
  world?: string | null;
  region?: string | null;
  site?: string | null;
  movement_type?: string;
  source_site?: string | null;
  importance?: number | null;
}

export interface CharacterEntry {
  name: string;
  action: string;
  role?: string | null;
  description?: string | null;
  importance?: number | null;
}

export interface AcquiredFrom {
  spatial?: string | null;
  temporal?: string | null;
}

export interface ItemSkillEntry {
  name: string;
  category: string;
  acquired_from?: AcquiredFrom | null;
  usage_in_chapter?: string | null;
  future_hook_flag: boolean;
  importance?: number | null;
}

export interface SegmentExtraction {
  chapter_index: number;
  chapter_title: string;
  narrative_perspective?: string | null;
  temporal?: Temporal | null;
  spatial?: Spatial | null;
  characters: CharacterEntry[];
  items_and_skills: ItemSkillEntry[];
  plot_summary: string;
}

export interface ExtractionRequest {
  book_id: number;
  llm: LlmConfig;
  chunk_chars?: number | null;
  skip_extracted?: boolean;
  concurrency?: number | null;
}

export interface GpuInfo {
  name: string;
  total_bytes: number;
  used_bytes: number;
}

export interface GpuStatus {
  gpus: GpuInfo[];
  model_vram_bytes?: number | null;
  suggested_concurrency: number;
}

export const ENGINE_PRESETS: Record<EngineKind, { label: string; baseUrl: string }> = {
  lm_studio: { label: "LM Studio", baseUrl: "http://localhost:1234/v1" },
  ollama: { label: "Ollama", baseUrl: "http://localhost:11434/v1" },
  custom: { label: "自定义端点", baseUrl: "http://localhost:8080/v1" },
};

export const MODEL_CHIPS = ["qwen2.5-0.5b", "qwen2.5-1.5b", "qwen2.5-3b", "gemma-2-2b", "gemma-2b-it"];
