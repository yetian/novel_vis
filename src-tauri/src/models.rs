use serde::{Deserialize, Serialize};

/// ============================================================
/// 抽取结果的严格 JSON Schema 映射
/// （与 design.md 2.2 / README 中的契约保持一致）
/// ============================================================

fn default_uncertain() -> String {
    "UNCERTAIN".to_string()
}
fn default_stay() -> String {
    "STAY".to_string()
}
fn default_active() -> String {
    "ACTIVE".to_string()
}
fn default_item_cat() -> String {
    "ITEM".to_string()
}

/// 时间维度：CONTINUOUS | JUMP_FORWARD | FLASHBACK | UNCERTAIN
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Temporal {
    #[serde(rename = "type", default = "default_uncertain")]
    pub temporal_type: String,
    #[serde(default)]
    pub raw_marker: Option<String>,
    #[serde(default)]
    pub relative_delta_days: Option<i64>,
    #[serde(default)]
    pub reference_anchor: Option<String>,
}

/// 空间维度
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spatial {
    #[serde(default)]
    pub world: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub site: Option<String>,
    #[serde(default = "default_stay")]
    pub movement_type: String,
    #[serde(default)]
    pub source_site: Option<String>,
}

/// 人物条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterEntry {
    pub name: String,
    #[serde(default = "default_active")]
    pub action: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcquiredFrom {
    #[serde(default)]
    pub spatial: Option<String>,
    #[serde(default)]
    pub temporal: Option<String>,
}

/// 物品 / 功法 / 传承
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemSkillEntry {
    pub name: String,
    #[serde(default = "default_item_cat")]
    pub category: String,
    #[serde(default)]
    pub acquired_from: Option<AcquiredFrom>,
    #[serde(default)]
    pub usage_in_chapter: Option<String>,
    #[serde(default)]
    pub future_hook_flag: bool,
}

/// 单个章节/分段的核心抽取结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentExtraction {
    #[serde(default)]
    pub chapter_index: i64,
    #[serde(default)]
    pub chapter_title: String,
    #[serde(default)]
    pub narrative_perspective: Option<String>,
    #[serde(default)]
    pub temporal: Option<Temporal>,
    #[serde(default)]
    pub spatial: Option<Spatial>,
    #[serde(default)]
    pub characters: Vec<CharacterEntry>,
    #[serde(default)]
    pub items_and_skills: Vec<ItemSkillEntry>,
    #[serde(default)]
    pub plot_summary: String,
}

/// ============================================================
/// 解析层模型（内存中的电子书）
/// ============================================================

#[derive(Debug, Clone, Serialize)]
pub struct ParsedChapter {
    pub chapter_index: i64,
    pub title: String,
    pub text: String,
    pub content_hash: String,
    pub char_count: usize,
}

/// 单本电子书中可能包含的候选书目（用于合订本/合集的拆分确认）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookCandidate {
    pub title: String,
    pub start_index: i64,
    pub end_index: i64,
    pub chapter_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ParsedBook {
    pub title: String,
    pub author: Option<String>,
    pub format: String,
    pub chapters: Vec<ParsedChapter>,
    /// 由目录结构推断出的候选书目；长度 >= 2 时提示可能为合订本。
    pub candidates: Vec<BookCandidate>,
}

/// 导入前的文件探查结果，供前端弹出「多本 / 单本」确认。
#[derive(Debug, Clone, Serialize)]
pub struct BookInspection {
    pub title: String,
    pub author: Option<String>,
    pub format: String,
    pub total_chapters: usize,
    pub suspected_multi_book: bool,
    pub signals: Vec<String>,
    pub candidates: Vec<BookCandidate>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IngestResult {
    pub book_id: i64,
    pub title: String,
    pub author: Option<String>,
    pub format: String,
    pub total_chapters: usize,
}

/// ============================================================
/// 数据库读取模型（供前端画布使用）
/// ============================================================

#[derive(Debug, Clone, Serialize)]
pub struct BookRow {
    pub id: i64,
    pub title: String,
    pub author: Option<String>,
    pub format: String,
    pub total_chapters: i64,
    pub created_at: String,
    pub extracted_chapters: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChapterRow {
    pub id: i64,
    pub book_id: i64,
    pub chapter_index: i64,
    pub title: String,
    pub raw_text_summary: Option<String>,
    pub temporal_type: Option<String>,
    pub time_offset_days: Option<i64>,
    pub world: Option<String>,
    pub region: Option<String>,
    pub site: Option<String>,
    pub movement_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EntityRow {
    pub id: i64,
    pub book_id: i64,
    pub entity_type: String,
    pub name: String,
    pub first_appeared_chapter_id: Option<i64>,
    pub first_appeared_chapter_index: Option<i64>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EntityEventRow {
    pub id: i64,
    pub entity_id: i64,
    pub chapter_id: i64,
    pub chapter_index: i64,
    pub action_type: String,
    pub role: Option<String>,
    pub acquired_site: Option<String>,
    pub impact_detail: Option<String>,
}

/// 画布聚合数据
#[derive(Debug, Clone, Serialize)]
pub struct TimelineBundle {
    pub book: BookRow,
    pub chapters: Vec<ChapterRow>,
    pub entities: Vec<EntityRow>,
    pub events: Vec<EntityEventRow>,
}

/// ============================================================
/// LLM 配置（支持 LM Studio / Ollama / 自定义 OpenAI 兼容端点）
/// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineKind {
    LmStudio,
    Ollama,
    Custom,
}

impl Default for EngineKind {
    fn default() -> Self {
        EngineKind::LmStudio
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    #[serde(default)]
    pub engine: EngineKind,
    /// OpenAI 兼容根地址，例如 http://localhost:1234/v1
    pub base_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    pub model: String,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

fn default_temperature() -> f32 {
    0.1
}
fn default_max_tokens() -> u32 {
    3072
}
fn default_timeout() -> u64 {
    180
}
fn default_max_retries() -> u32 {
    3
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub owned_by: Option<String>,
}

/// 抽取日志事件（按书籍区分，支持多本并发抽取）
#[derive(Debug, Clone, Serialize)]
pub struct LogEvent {
    pub book_id: i64,
    pub message: String,
}

/// 抽取任务进度事件
#[derive(Debug, Clone, Serialize)]
pub struct ProgressEvent {
    pub book_id: i64,
    pub phase: String,
    pub current: usize,
    pub total: usize,
    pub chapter_index: i64,
    pub chapter_title: String,
    pub message: String,
    pub retries: u32,
}

/// 单章抽取结果（含修复元信息）
#[derive(Debug, Clone, Serialize)]
pub struct ChapterExtractionResult {
    pub chapter_index: i64,
    pub chapter_title: String,
    pub ok: bool,
    pub retries_used: u32,
    pub repaired: bool,
    pub extraction: Option<SegmentExtraction>,
    pub error: Option<String>,
}
