import type { ChapterRow } from "@/types/schema";

const SEPARATORS = [" - ", " – ", " — ", " | ", "·", "_", "：", ":"];

function normalize(s: string): string {
  return s.replace(/\s+/g, "").toLowerCase();
}

/// 章节标题展示：旧数据可能把书名写进章节标题，这里做兜底清洗。
export function chapterLabel(
  chapter: Pick<ChapterRow, "title" | "chapter_index">,
  bookTitle?: string | null,
): string {
  const raw = (chapter.title || "").trim();
  const book = (bookTitle || "").trim();
  if (!raw) return `第 ${chapter.chapter_index + 1} 章`;
  if (!book) return raw;
  const bookKey = normalize(book);
  if (normalize(raw) === bookKey) return `第 ${chapter.chapter_index + 1} 章`;
  for (const sep of SEPARATORS) {
    const parts = raw.split(sep);
    if (parts.length !== 2) continue;
    const left = parts[0].trim();
    const right = parts[1].trim();
    if (!left || !right) continue;
    if (normalize(left) === bookKey) return right;
    if (normalize(right) === bookKey) return left;
  }
  return raw;
}

/// 真正的章节名；若只能回退到「第 N 章」，返回空串，避免与章节序号重复展示。
export function chapterHeading(
  chapter: Pick<ChapterRow, "title" | "chapter_index">,
  bookTitle?: string | null,
): string {
  const label = chapterLabel(chapter, bookTitle);
  return label === `第 ${chapter.chapter_index + 1} 章` ? "" : label;
}
