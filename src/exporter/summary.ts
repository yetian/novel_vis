import { chapterHeading } from "@/utils/chapter";
import { computeImportance } from "@/utils/people";
import { cleanPlace, cleanWorld } from "@/utils/space";
import type { BookRow, ChapterRow, EntityEventRow, EntityRow } from "@/types/schema";

export interface SummaryExportInput {
  book: BookRow;
  summary: string;
  chapters: ChapterRow[];
  entities: EntityRow[];
  events: EntityEventRow[];
}

interface ChapterPerson {
  id: number;
  name: string;
  role: string;
  importance: number;
  event_count: number;
}

interface ExportChapter {
  chapter_index: number;
  title: string;
  summary: string;
  time: {
    type: string | null;
    offset_days: number | null;
    story_day: number;
    detected: boolean;
  };
  space: {
    world: string | null;
    region: string | null;
    site: string | null;
  };
  main_characters: ChapterPerson[];
  first_appearances: Omit<ChapterPerson, "event_count">[];
  event_count: number;
}

interface CharacterStats {
  firstByChapter: Map<number, EntityRow[]>;
  mainByChapter: Map<number, ChapterPerson[]>;
  roleByChar: Map<number, string>;
  importanceById: Map<number, number>;
  eventsByChar: Map<number, EntityEventRow[]>;
}

function buildCharacterStats(
  chapters: ChapterRow[],
  entities: EntityRow[],
  events: EntityEventRow[],
): CharacterStats {
  const characterEntities = entities.filter((e) => e.entity_type === "CHARACTER");
  const characterIds = new Set(characterEntities.map((e) => e.id));
  const eventsByChar = new Map<number, EntityEventRow[]>();
  for (const event of events) {
    if (!characterIds.has(event.entity_id)) continue;
    const list = eventsByChar.get(event.entity_id) ?? [];
    list.push(event);
    eventsByChar.set(event.entity_id, list);
  }

  const roleCounts = new Map<number, Map<string, number>>();
  for (const event of events) {
    if (!event.role) continue;
    let counts = roleCounts.get(event.entity_id);
    if (!counts) {
      counts = new Map();
      roleCounts.set(event.entity_id, counts);
    }
    counts.set(event.role, (counts.get(event.role) ?? 0) + 1);
  }
  const roleByChar = new Map<number, string>();
  for (const [id, counts] of roleCounts) {
    const best = [...counts.entries()].sort((a, b) => b[1] - a[1])[0];
    if (best) roleByChar.set(id, best[0]);
  }

  const maxEvents = characterEntities.reduce(
    (max, entity) => Math.max(max, (eventsByChar.get(entity.id) ?? []).length),
    1,
  );
  const totalChapters = Math.max(1, chapters.length);
  const totalSpan = Math.max(1, chapters.length - 1);
  const importanceById = new Map<number, number>();
  for (const entity of characterEntities) {
    const evs = eventsByChar.get(entity.id) ?? [];
    const chapterSet = new Set(evs.map((event) => event.chapter_index));
    let first = entity.first_appeared_chapter_index ?? null;
    let last = first;
    for (const event of evs) {
      if (first == null || event.chapter_index < first) first = event.chapter_index;
      if (last == null || event.chapter_index > last) last = event.chapter_index;
    }
    const span = first != null && last != null ? Math.max(0, last - first) : 0;
    const explicit =
      entity.importance != null && entity.importance >= 1 && entity.importance <= 5
        ? Math.round(entity.importance)
        : null;
    importanceById.set(
      entity.id,
      computeImportance({
        explicit,
        coverage: chapterSet.size / totalChapters,
        volume: evs.length / maxEvents,
        spanRatio: span / totalSpan,
        role: roleByChar.get(entity.id),
      }),
    );
  }

  const firstByChapter = new Map<number, EntityRow[]>();
  for (const entity of characterEntities) {
    if (entity.first_appeared_chapter_index == null) continue;
    const list = firstByChapter.get(entity.first_appeared_chapter_index) ?? [];
    list.push(entity);
    firstByChapter.set(entity.first_appeared_chapter_index, list);
  }
  for (const list of firstByChapter.values()) {
    list.sort(
      (a, b) =>
        (importanceById.get(b.id) ?? 1) - (importanceById.get(a.id) ?? 1) ||
        a.name.localeCompare(b.name),
    );
  }

  const byChapter = new Map<number, Map<number, number>>();
  for (const event of events) {
    if (!characterIds.has(event.entity_id)) continue;
    let counts = byChapter.get(event.chapter_index);
    if (!counts) {
      counts = new Map();
      byChapter.set(event.chapter_index, counts);
    }
    counts.set(event.entity_id, (counts.get(event.entity_id) ?? 0) + 1);
  }
  const mainByChapter = new Map<number, ChapterPerson[]>();
  const entityById = new Map(entities.map((entity) => [entity.id, entity]));
  for (const chapter of chapters) {
    const counts = byChapter.get(chapter.chapter_index);
    if (!counts?.size) continue;
    const firstIds = new Set(
      (firstByChapter.get(chapter.chapter_index) ?? []).map((entity) => entity.id),
    );
    const list = [...counts.entries()]
      .map(([id, count]) => {
        const entity = entityById.get(id);
        return entity
          ? {
              id,
              name: entity.name,
              role: roleByChar.get(id) ?? "",
              importance: importanceById.get(id) ?? 1,
              event_count: count,
            }
          : null;
      })
      .filter((item): item is ChapterPerson => !!item)
      .filter((item) => !firstIds.has(item.id))
      .sort(
        (a, b) =>
          b.importance - a.importance ||
          b.event_count - a.event_count ||
          a.name.localeCompare(b.name),
      )
      .slice(0, 6);
    if (list.length) mainByChapter.set(chapter.chapter_index, list);
  }

  return { firstByChapter, mainByChapter, roleByChar, importanceById, eventsByChar };
}

function firstAppearances(
  chapterIndex: number,
  stats: CharacterStats,
): Omit<ChapterPerson, "event_count">[] {
  return (stats.firstByChapter.get(chapterIndex) ?? []).map((entity) => ({
    id: entity.id,
    name: entity.name,
    role: stats.roleByChar.get(entity.id) ?? "",
    importance: stats.importanceById.get(entity.id) ?? 1,
  }));
}

export function buildSummaryExport(input: SummaryExportInput) {
  const { book, summary, chapters, entities, events } = input;
  const stats = buildCharacterStats(chapters, entities, events);
  const chapterByIndex = new Map(chapters.map((c) => [c.chapter_index, c]));
  const eventsByChapter = new Map<number, EntityEventRow[]>();
  for (const event of events) {
    const list = eventsByChapter.get(event.chapter_index) ?? [];
    list.push(event);
    eventsByChapter.set(event.chapter_index, list);
  }

  let storyDay = 0;
  const exportChapters: ExportChapter[] = chapters.map((chapter) => {
    storyDay += chapter.time_offset_days ?? 0;
    return {
      chapter_index: chapter.chapter_index,
      title: chapterHeading(chapter, book.title),
      summary: chapter.raw_text_summary ?? "",
      time: {
        type: chapter.temporal_type ?? null,
        offset_days: chapter.time_offset_days ?? null,
        story_day: storyDay,
        detected:
          (!!chapter.temporal_type && chapter.temporal_type !== "UNCERTAIN") ||
          (chapter.time_offset_days ?? 0) !== 0,
      },
      space: {
        world: cleanWorld(chapter.world) || null,
        region: cleanPlace(chapter.region) || null,
        site: cleanPlace(chapter.site) || null,
      },
      main_characters: stats.mainByChapter.get(chapter.chapter_index) ?? [],
      first_appearances: firstAppearances(chapter.chapter_index, stats),
      event_count: (eventsByChapter.get(chapter.chapter_index) ?? []).length,
    };
  });

  const characters = entities
    .filter((entity) => entity.entity_type === "CHARACTER")
    .map((entity) => {
      const evs = stats.eventsByChar.get(entity.id) ?? [];
      const chapterIndexes = [...new Set(evs.map((event) => event.chapter_index))].sort(
        (a, b) => a - b,
      );
      const firstIndex = entity.first_appeared_chapter_index ?? chapterIndexes[0] ?? null;
      const lastIndex = chapterIndexes.length ? chapterIndexes[chapterIndexes.length - 1] : firstIndex;
      const firstChapter = firstIndex != null ? chapterByIndex.get(firstIndex) ?? null : null;
      const lastChapter = lastIndex != null ? chapterByIndex.get(lastIndex) ?? null : null;
      return {
        id: entity.id,
        name: entity.name,
        role: stats.roleByChar.get(entity.id) ?? "",
        description: entity.description ?? "",
        importance: stats.importanceById.get(entity.id) ?? 1,
        first_chapter: firstIndex,
        first_chapter_title: firstChapter ? chapterHeading(firstChapter, book.title) : null,
        last_chapter: lastIndex,
        last_chapter_title: lastChapter ? chapterHeading(lastChapter, book.title) : null,
        chapters: chapterIndexes,
        event_count: evs.length,
      };
    })
    .sort((a, b) => b.importance - a.importance || a.name.localeCompare(b.name));

  const eventNameById = new Map(entities.map((entity) => [entity.id, entity]));
  const locationMap = new Map<
    string,
    {
      world: string;
      region: string;
      site: string;
      chapters: number[];
      characters: Set<string>;
      items: Set<string>;
      eventCount: number;
    }
  >();
  const chapterSpaceKey = new Map<number, string>();
  for (const chapter of chapters) {
    const world = cleanWorld(chapter.world);
    const region = cleanPlace(chapter.region);
    const site = cleanPlace(chapter.site) || region || world;
    if (!site) continue;
    const key = `${world}::${region}::${site}`;
    chapterSpaceKey.set(chapter.chapter_index, key);
    const location = locationMap.get(key) ?? {
      world,
      region,
      site,
      chapters: [],
      characters: new Set<string>(),
      items: new Set<string>(),
      eventCount: 0,
    };
    location.chapters.push(chapter.chapter_index);
    locationMap.set(key, location);
  }
  for (const event of events) {
    const key = chapterSpaceKey.get(event.chapter_index);
    if (!key) continue;
    const location = locationMap.get(key);
    const entity = eventNameById.get(event.entity_id);
    if (!location || !entity) continue;
    location.eventCount += 1;
    if (entity.entity_type === "CHARACTER") location.characters.add(entity.name);
    else location.items.add(entity.name);
  }
  const locations = [...locationMap.values()]
    .map((location) => ({
      ...location,
      chapters: [...new Set(location.chapters)].sort((a, b) => a - b),
      characters: [...location.characters].sort(),
      items: [...location.items].sort(),
      chapter_count: new Set(location.chapters).size,
    }))
    .sort(
      (a, b) =>
        b.chapter_count - a.chapter_count ||
        b.eventCount - a.eventCount ||
        a.site.localeCompare(b.site),
    );

  const items = entities
    .filter((entity) => entity.entity_type !== "CHARACTER")
    .map((entity) => {
      const evs = events.filter((event) => event.entity_id === entity.id);
      const chapterIndexes = [...new Set(evs.map((event) => event.chapter_index))].sort(
        (a, b) => a - b,
      );
      return {
        id: entity.id,
        name: entity.name,
        type: entity.entity_type,
        description: entity.description ?? "",
        importance: entity.importance ?? null,
        acquired_site:
          evs.find((event) => event.action_type === "ACQUIRE")?.acquired_site ?? null,
        first_chapter: chapterIndexes[0] ?? null,
        last_chapter: chapterIndexes[chapterIndexes.length - 1] ?? null,
        chapters: chapterIndexes,
        event_count: evs.length,
      };
    })
    .sort((a, b) => (b.importance ?? 0) - (a.importance ?? 0) || a.name.localeCompare(b.name));

  const exportEvents = events.map((event) => {
    const entity = eventNameById.get(event.entity_id);
    return {
      chapter_index: event.chapter_index,
      entity: entity?.name ?? null,
      entity_type: entity?.entity_type ?? null,
      action: event.action_type,
      role: event.role ?? null,
      acquired_site: event.acquired_site ?? null,
      impact_detail: event.impact_detail ?? null,
    };
  });

  return {
    book: {
      id: book.id,
      title: book.title,
      author: book.author ?? null,
      format: book.format,
      total_chapters: book.total_chapters,
      summary,
    },
    chapters: exportChapters,
    characters,
    locations,
    items,
    events: exportEvents,
  };
}

export function summaryJson(input: SummaryExportInput): string {
  return JSON.stringify(buildSummaryExport(input), null, 2);
}

function csvCell(value: unknown): string {
  const text = value == null ? "" : String(value);
  return /[;"\r\n]/.test(text) ? `"${text.replace(/"/g, '""')}"` : text;
}

export function summaryCsv(input: SummaryExportInput): string {
  const { book, chapters, entities, events } = input;
  const stats = buildCharacterStats(chapters, entities, events);
  let storyDay = 0;
  const header = [
    "chapter_index",
    "chapter_title",
    "summary",
    "world",
    "region",
    "site",
    "temporal_type",
    "time_offset_days",
    "story_day",
    "time_detected",
    "main_characters",
    "first_appearances",
  ].join(";");
  const rows = chapters.map((chapter) => {
    storyDay += chapter.time_offset_days ?? 0;
    const first = firstAppearances(chapter.chapter_index, stats);
    const detected =
      (!!chapter.temporal_type && chapter.temporal_type !== "UNCERTAIN") ||
      (chapter.time_offset_days ?? 0) !== 0;
    return [
      chapter.chapter_index,
      chapterHeading(chapter, book.title),
      chapter.raw_text_summary ?? "",
      cleanWorld(chapter.world),
      cleanPlace(chapter.region),
      cleanPlace(chapter.site),
      chapter.temporal_type ?? "",
      chapter.time_offset_days ?? "",
      storyDay,
      detected ? "1" : "0",
      (stats.mainByChapter.get(chapter.chapter_index) ?? []).map((person) => person.name).join("、"),
      first.map((person) => person.name).join("、"),
    ]
      .map(csvCell)
      .join(";");
  });
  return `\uFEFF${header}\r\n${rows.join("\r\n")}\r\n`;
}
