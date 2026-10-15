import { invoke } from "@tauri-apps/api/core";
import type {
  BookRow,
  ChapterExtractionResult,
  ExtractionRequest,
  IngestResult,
  LlmConfig,
  ModelInfo,
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
  ingestBook: (path: string) => guard(() => invoke<IngestResult>("ingest_book", { path })),
  listBooks: () => guard(() => invoke<BookRow[]>("list_books")),
  getTimeline: (bookId: number) => guard(() => invoke<TimelineBundle>("get_timeline", { bookId })),
  deleteBook: (bookId: number) => guard(() => invoke<void>("delete_book", { bookId })),
  saveLlmConfig: (config: LlmConfig) => guard(() => invoke<void>("save_llm_config", { config })),
  getLlmConfig: () => guard(() => invoke<LlmConfig | null>("get_llm_config")),
  listModels: (config: LlmConfig) => guard(() => invoke<ModelInfo[]>("list_models", { config })),
  testConnection: (config: LlmConfig) => guard(() => invoke<boolean>("test_connection", { config })),
  startExtraction: (request: ExtractionRequest) =>
    guard(() => invoke<ChapterExtractionResult[]>("start_extraction", { request })),
  pauseExtraction: (bookId: number) => guard(() => invoke<void>("pause_extraction", { bookId })),
  resumeExtraction: (bookId: number) => guard(() => invoke<void>("resume_extraction", { bookId })),
  cancelExtraction: (bookId: number) => guard(() => invoke<void>("cancel_extraction", { bookId })),
  readTextFile: (path: string) => guard(() => invoke<string>("read_text_file", { path })),
};
