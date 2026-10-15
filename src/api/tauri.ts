import { invoke } from "@tauri-apps/api/core";
import type {
  BookInspection,
  BookRow,
  ChapterExtractionResult,
  ExtractionRequest,
  IngestResult,
  LlmConfig,
  ModelInfo,
  SourceCheck,
  TimelineBundle,
} from "@/types/schema";

// 判断是否运行在 Tauri 宿主中（便于在纯浏览器下优雅降级）。
export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

function guard<T>(fn: () => Promise<T>): Promise<T> {
  if (!isTauri) {
    return Promise.reject(new Error("当前不在 Tauri 环境中运行"));
  }
  return fn();
}

export const api = {
  inspectBook: (path: string) => guard(() => invoke<BookInspection>("inspect_book", { path })),
  ingestBook: (path: string, splitIntoBooks = false) =>
    guard(() => invoke<IngestResult[]>("ingest_book", { path, splitIntoBooks })),
  listBooks: () => guard(() => invoke<BookRow[]>("list_books")),
  getTimeline: (bookId: number) => guard(() => invoke<TimelineBundle>("get_timeline", { bookId })),
  deleteBook: (bookId: number) => guard(() => invoke<void>("delete_book", { bookId })),
  clearAllData: () => guard(() => invoke<void>("clear_all_data")),
  setBookSource: (bookId: number, path: string, force = false) =>
    guard(() => invoke<SourceCheck>("set_book_source", { bookId, path, force })),
  getBookSource: (bookId: number) => guard(() => invoke<string | null>("get_book_source", { bookId })),
  saveLlmConfig: (config: LlmConfig) => guard(() => invoke<void>("save_llm_config", { config })),
  getLlmConfig: () => guard(() => invoke<LlmConfig | null>("get_llm_config")),
  listModels: (config: LlmConfig) => guard(() => invoke<ModelInfo[]>("list_models", { config })),
  listLoadedModels: (config: LlmConfig) => guard(() => invoke<ModelInfo[]>("list_loaded_models", { config })),
  activateModel: (config: LlmConfig) => guard(() => invoke<string>("activate_model", { config })),
  testConnection: (config: LlmConfig) => guard(() => invoke<boolean>("test_connection", { config })),
  startExtraction: (request: ExtractionRequest) =>
    guard(() => invoke<ChapterExtractionResult[]>("start_extraction", { request })),
  pauseExtraction: (bookId: number) => guard(() => invoke<void>("pause_extraction", { bookId })),
  resumeExtraction: (bookId: number) => guard(() => invoke<void>("resume_extraction", { bookId })),
  cancelExtraction: (bookId: number) => guard(() => invoke<void>("cancel_extraction", { bookId })),
  readTextFile: (path: string) => guard(() => invoke<string>("read_text_file", { path })),
  getDataDir: () => guard(() => invoke<string>("get_data_dir")),
};
