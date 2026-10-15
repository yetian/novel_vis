use crate::error::AppResult;
use crate::models::*;
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

/// 全局数据库句柄。内部用 `Mutex` 串行化访问，满足 Tauri `State` 的 `Send + Sync`。
pub struct Db {
    pub conn: Mutex<Connection>,
}

#[allow(dead_code)]
impl Db {
    pub fn open(path: &Path) -> AppResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        Self::from_conn(conn)
    }

    pub fn open_in_memory() -> AppResult<Self> {
        Self::from_conn(Connection::open_in_memory()?)
    }

    fn from_conn(conn: Connection) -> AppResult<Self> {
        conn.pragma_update(None, "journal_mode", "WAL").ok();
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "synchronous", "NORMAL").ok();
        let db = Db {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    /// 幂等迁移：所有建表语句都使用 IF NOT EXISTS，可安全重复执行。
    pub fn migrate(&self) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS books (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                title           TEXT NOT NULL,
                author          TEXT,
                format          TEXT NOT NULL,
                total_chapters  INTEGER NOT NULL DEFAULT 0,
                created_at      TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS chapters (
                id               INTEGER PRIMARY KEY AUTOINCREMENT,
                book_id          INTEGER NOT NULL REFERENCES books(id) ON DELETE CASCADE,
                chapter_index    INTEGER NOT NULL,
                title            TEXT NOT NULL,
                content_hash     TEXT NOT NULL,
                raw_text_summary TEXT,
                UNIQUE(book_id, chapter_index)
            );
            CREATE INDEX IF NOT EXISTS idx_chapters_book ON chapters(book_id, chapter_index);

            CREATE TABLE IF NOT EXISTS spatiotemporal_nodes (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                chapter_id      INTEGER NOT NULL REFERENCES chapters(id) ON DELETE CASCADE,
                book_id         INTEGER NOT NULL REFERENCES books(id) ON DELETE CASCADE,
                temporal_type   TEXT,
                time_offset_days INTEGER,
                world           TEXT,
                region          TEXT,
                site            TEXT,
                movement_type   TEXT,
                raw_json        TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_nodes_book ON spatiotemporal_nodes(book_id);
            CREATE INDEX IF NOT EXISTS idx_nodes_chapter ON spatiotemporal_nodes(chapter_id);

            CREATE TABLE IF NOT EXISTS entities (
                id                        INTEGER PRIMARY KEY AUTOINCREMENT,
                book_id                   INTEGER NOT NULL REFERENCES books(id) ON DELETE CASCADE,
                type                      TEXT NOT NULL,
                name                      TEXT NOT NULL,
                first_appeared_chapter_id INTEGER REFERENCES chapters(id) ON DELETE SET NULL,
                role                      TEXT,
                description               TEXT,
                UNIQUE(book_id, type, name)
            );
            CREATE INDEX IF NOT EXISTS idx_entities_book ON entities(book_id);

            CREATE TABLE IF NOT EXISTS entity_events (
                id             INTEGER PRIMARY KEY AUTOINCREMENT,
                entity_id      INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
                chapter_id     INTEGER NOT NULL REFERENCES chapters(id) ON DELETE CASCADE,
                action_type    TEXT NOT NULL,
                role           TEXT,
                acquired_site  TEXT,
                impact_detail  TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_events_entity ON entity_events(entity_id);
            CREATE INDEX IF NOT EXISTS idx_events_chapter ON entity_events(chapter_id);
            "#,
        )?;
        Ok(())
    }

    // -------- 写入 --------

    pub fn insert_book(
        &self,
        title: &str,
        author: Option<&str>,
        format: &str,
        total_chapters: i64,
    ) -> AppResult<i64> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO books (title, author, format, total_chapters) VALUES (?1, ?2, ?3, ?4)",
            params![title, author, format, total_chapters],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn insert_chapter(
        &self,
        book_id: i64,
        chapter_index: i64,
        title: &str,
        content_hash: &str,
        summary: Option<&str>,
    ) -> AppResult<i64> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR IGNORE INTO chapters (book_id, chapter_index, title, content_hash, raw_text_summary)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![book_id, chapter_index, title, content_hash, summary],
        )?;
        let id: Option<i64> = conn
            .query_row(
                "SELECT id FROM chapters WHERE book_id = ?1 AND chapter_index = ?2",
                params![book_id, chapter_index],
                |r| r.get(0),
            )
            .optional()?;
        id.ok_or_else(|| crate::error::AppError::Internal("章节写入失败".into()))
    }

    pub fn set_chapter_summary(&self, chapter_id: i64, summary: &str) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE chapters SET raw_text_summary = ?1 WHERE id = ?2",
            params![summary, chapter_id],
        )?;
        Ok(())
    }

    pub fn insert_node(
        &self,
        chapter_id: i64,
        book_id: i64,
        temporal_type: Option<&str>,
        time_offset_days: Option<i64>,
        world: Option<&str>,
        region: Option<&str>,
        site: Option<&str>,
        movement_type: Option<&str>,
        raw_json: &str,
    ) -> AppResult<i64> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO spatiotemporal_nodes
                (chapter_id, book_id, temporal_type, time_offset_days, world, region, site, movement_type, raw_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                chapter_id,
                book_id,
                temporal_type,
                time_offset_days,
                world,
                region,
                site,
                movement_type,
                raw_json
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// 实体采用 upsert 语义，返回实体 id。
    pub fn upsert_entity(
        &self,
        book_id: i64,
        entity_type: &str,
        name: &str,
        first_appeared_chapter_id: Option<i64>,
        role: Option<&str>,
        description: Option<&str>,
    ) -> AppResult<i64> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO entities (book_id, type, name, first_appeared_chapter_id, role, description)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(book_id, type, name) DO UPDATE SET
                first_appeared_chapter_id = COALESCE(entities.first_appeared_chapter_id, excluded.first_appeared_chapter_id),
                role = COALESCE(excluded.role, entities.role),
                description = COALESCE(excluded.description, entities.description)",
            params![
                book_id,
                entity_type,
                name,
                first_appeared_chapter_id,
                role,
                description
            ],
        )?;
        let id: i64 = conn.query_row(
            "SELECT id FROM entities WHERE book_id = ?1 AND type = ?2 AND name = ?3",
            params![book_id, entity_type, name],
            |r| r.get(0),
        )?;
        Ok(id)
    }

    pub fn insert_event(
        &self,
        entity_id: i64,
        chapter_id: i64,
        action_type: &str,
        role: Option<&str>,
        acquired_site: Option<&str>,
        impact_detail: Option<&str>,
    ) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO entity_events (entity_id, chapter_id, action_type, role, acquired_site, impact_detail)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![entity_id, chapter_id, action_type, role, acquired_site, impact_detail],
        )?;
        Ok(())
    }

    /// 判断某章节是否已完成抽取（用于断点续跑）。
    pub fn chapter_extracted(&self, chapter_id: i64) -> AppResult<bool> {
        let conn = self.conn.lock();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM spatiotemporal_nodes WHERE chapter_id = ?1",
            params![chapter_id],
            |r| r.get(0),
        )?;
        Ok(count > 0)
    }

    /// 将单章抽取结果写入数据库。整个过程在一个事务内完成，
    /// 作为“事务检查点”，保证暂停/崩溃后可安全续跑。
    pub fn persist_extraction(
        &self,
        book_id: i64,
        chapter_id: i64,
        ext: &SegmentExtraction,
    ) -> AppResult<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;

        let raw = serde_json::to_string(ext)?;
        let (temporal_type, time_offset, world, region, site, movement) = match &ext.temporal {
            Some(t) => {
                let s = &ext.spatial;
                (
                    Some(t.temporal_type.clone()),
                    t.relative_delta_days,
                    s.as_ref().and_then(|x| x.world.clone()),
                    s.as_ref().and_then(|x| x.region.clone()),
                    s.as_ref().and_then(|x| x.site.clone()),
                    s.as_ref().map(|x| x.movement_type.clone()),
                )
            }
            None => {
                let s = ext.spatial.as_ref();
                (
                    None,
                    None,
                    s.and_then(|x| x.world.clone()),
                    s.and_then(|x| x.region.clone()),
                    s.and_then(|x| x.site.clone()),
                    s.map(|x| x.movement_type.clone()),
                )
            }
        };

        tx.execute(
            "INSERT INTO spatiotemporal_nodes
                (chapter_id, book_id, temporal_type, time_offset_days, world, region, site, movement_type, raw_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                chapter_id,
                book_id,
                temporal_type,
                time_offset,
                world,
                region,
                site,
                movement,
                raw
            ],
        )?;

        for c in &ext.characters {
            upsert_entity_tx(&tx, book_id, "CHARACTER", &c.name, chapter_id, c.role.as_deref(), c.description.as_deref())?;
            let eid = entity_id_tx(&tx, book_id, "CHARACTER", &c.name)?;
            tx.execute(
                "INSERT INTO entity_events (entity_id, chapter_id, action_type, role, acquired_site, impact_detail)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![eid, chapter_id, c.action, c.role, Option::<String>::None, c.description],
            )?;
        }

        for it in &ext.items_and_skills {
            upsert_entity_tx(&tx, book_id, &it.category, &it.name, chapter_id, None, it.usage_in_chapter.as_deref())?;
            let eid = entity_id_tx(&tx, book_id, &it.category, &it.name)?;
            let site = it.acquired_from.as_ref().and_then(|a| a.spatial.clone());
            tx.execute(
                "INSERT INTO entity_events (entity_id, chapter_id, action_type, role, acquired_site, impact_detail)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![eid, chapter_id, "ACQUIRE", Option::<String>::None, site, it.usage_in_chapter],
            )?;
        }

        if !ext.plot_summary.trim().is_empty() {
            tx.execute(
                "UPDATE chapters SET raw_text_summary = ?1 WHERE id = ?2",
                params![ext.plot_summary, chapter_id],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    // -------- 读取 --------

    pub fn list_books(&self) -> AppResult<Vec<BookRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT b.id, b.title, b.author, b.format, b.total_chapters, b.created_at,
                    (SELECT COUNT(DISTINCT n.chapter_id) FROM spatiotemporal_nodes n WHERE n.book_id = b.id)
             FROM books b ORDER BY b.created_at DESC",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(BookRow {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    author: r.get(2)?,
                    format: r.get(3)?,
                    total_chapters: r.get(4)?,
                    created_at: r.get(5)?,
                    extracted_chapters: r.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get_book(&self, book_id: i64) -> AppResult<BookRow> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT b.id, b.title, b.author, b.format, b.total_chapters, b.created_at,
                    (SELECT COUNT(DISTINCT n.chapter_id) FROM spatiotemporal_nodes n WHERE n.book_id = b.id)
             FROM books b WHERE b.id = ?1",
            params![book_id],
            |r| {
                Ok(BookRow {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    author: r.get(2)?,
                    format: r.get(3)?,
                    total_chapters: r.get(4)?,
                    created_at: r.get(5)?,
                    extracted_chapters: r.get(6)?,
                })
            },
        )
        .map_err(Into::into)
    }

    pub fn delete_book(&self, book_id: i64) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM books WHERE id = ?1", params![book_id])?;
        Ok(())
    }

    /// 清空全部数据（用于清除被污染或测试导入的数据）。按子表 → 父表顺序删除。
    pub fn delete_all(&self) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute_batch(
            "DELETE FROM entity_events;
             DELETE FROM entities;
             DELETE FROM spatiotemporal_nodes;
             DELETE FROM chapters;
             DELETE FROM books;
             DELETE FROM sqlite_sequence;",
        )
        .ok();
        Ok(())
    }

    pub fn get_chapters(&self, book_id: i64) -> AppResult<Vec<ChapterRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT c.id, c.book_id, c.chapter_index, c.title, c.raw_text_summary,
                    n.temporal_type, n.time_offset_days, n.world, n.region, n.site, n.movement_type
             FROM chapters c
             LEFT JOIN spatiotemporal_nodes n ON n.id = (
                 SELECT id FROM spatiotemporal_nodes
                 WHERE chapter_id = c.id
                 ORDER BY id ASC LIMIT 1
             )
             WHERE c.book_id = ?1
             ORDER BY c.chapter_index ASC",
        )?;
        let rows = stmt
            .query_map(params![book_id], |r| {
                Ok(ChapterRow {
                    id: r.get(0)?,
                    book_id: r.get(1)?,
                    chapter_index: r.get(2)?,
                    title: r.get(3)?,
                    raw_text_summary: r.get(4)?,
                    temporal_type: r.get(5)?,
                    time_offset_days: r.get(6)?,
                    world: r.get(7)?,
                    region: r.get(8)?,
                    site: r.get(9)?,
                    movement_type: r.get(10)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get_entities(&self, book_id: i64) -> AppResult<Vec<EntityRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT e.id, e.book_id, e.type, e.name, e.first_appeared_chapter_id,
                    c.chapter_index, e.description
             FROM entities e
             LEFT JOIN chapters c ON c.id = e.first_appeared_chapter_id
             WHERE e.book_id = ?1
             ORDER BY e.type, e.id",
        )?;
        let rows = stmt
            .query_map(params![book_id], |r| {
                Ok(EntityRow {
                    id: r.get(0)?,
                    book_id: r.get(1)?,
                    entity_type: r.get(2)?,
                    name: r.get(3)?,
                    first_appeared_chapter_id: r.get(4)?,
                    first_appeared_chapter_index: r.get(5)?,
                    description: r.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get_events(&self, book_id: i64) -> AppResult<Vec<EntityEventRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT ev.id, ev.entity_id, ev.chapter_id, c.chapter_index,
                    ev.action_type, ev.role, ev.acquired_site, ev.impact_detail
             FROM entity_events ev
             JOIN chapters c ON c.id = ev.chapter_id
             JOIN entities e ON e.id = ev.entity_id
             WHERE e.book_id = ?1
             ORDER BY c.chapter_index ASC, ev.id ASC",
        )?;
        let rows = stmt
            .query_map(params![book_id], |r| {
                Ok(EntityEventRow {
                    id: r.get(0)?,
                    entity_id: r.get(1)?,
                    chapter_id: r.get(2)?,
                    chapter_index: r.get(3)?,
                    action_type: r.get(4)?,
                    role: r.get(5)?,
                    acquired_site: r.get(6)?,
                    impact_detail: r.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn timeline_bundle(&self, book_id: i64) -> AppResult<TimelineBundle> {
        Ok(TimelineBundle {
            book: self.get_book(book_id)?,
            chapters: self.get_chapters(book_id)?,
            entities: self.get_entities(book_id)?,
            events: self.get_events(book_id)?,
        })
    }
}

/// 事务内 upsert 实体（首次出现的章节仅在为空时写入）。
fn upsert_entity_tx(
    tx: &rusqlite::Transaction<'_>,
    book_id: i64,
    entity_type: &str,
    name: &str,
    chapter_id: i64,
    role: Option<&str>,
    description: Option<&str>,
) -> AppResult<()> {
    tx.execute(
        "INSERT INTO entities (book_id, type, name, first_appeared_chapter_id, role, description)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(book_id, type, name) DO UPDATE SET
            first_appeared_chapter_id = COALESCE(entities.first_appeared_chapter_id, excluded.first_appeared_chapter_id),
            role = COALESCE(excluded.role, entities.role),
            description = COALESCE(excluded.description, entities.description)",
        params![book_id, entity_type, name, chapter_id, role, description],
    )?;
    Ok(())
}

fn entity_id_tx(
    tx: &rusqlite::Transaction<'_>,
    book_id: i64,
    entity_type: &str,
    name: &str,
) -> AppResult<i64> {
    let id: i64 = tx.query_row(
        "SELECT id FROM entities WHERE book_id = ?1 AND type = ?2 AND name = ?3",
        params![book_id, entity_type, name],
        |r| r.get(0),
    )?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_extraction() -> SegmentExtraction {
        serde_json::from_value(json!({
            "chapter_index": 0,
            "chapter_title": "第一章",
            "narrative_perspective": "third_person",
            "temporal": {"type":"JUMP_FORWARD","raw_marker":"三年后","relative_delta_days":1095},
            "spatial": {"world":"人界","region":"越国","site":"神手谷","movement_type":"LOCAL_MOVE","source_site":"广场"},
            "characters":[{"name":"韩立","action":"FIRST_APPEARANCE","role":"主角","description":"少年"}],
            "items_and_skills":[{"name":"掌天瓶","category":"ITEM","acquired_from":{"spatial":"后山","temporal":"入夜"},"usage_in_chapter":"拾取","future_hook_flag":true}],
            "plot_summary":"韩立拾得掌天瓶"
        }))
        .unwrap()
    }

    #[test]
    fn schema_persist_and_read_back() {
        let db = Db::open_in_memory().unwrap();
        let book_id = db.insert_book("测试书", Some("作者甲"), "EPUB", 1).unwrap();
        let ch_id = db.insert_chapter(book_id, 0, "第一章", "hash123", None).unwrap();
        assert!(!db.chapter_extracted(ch_id).unwrap());

        db.persist_extraction(book_id, ch_id, &sample_extraction()).unwrap();
        assert!(db.chapter_extracted(ch_id).unwrap());

        let bundle = db.timeline_bundle(book_id).unwrap();
        assert_eq!(bundle.chapters.len(), 1);
        let ch = &bundle.chapters[0];
        assert_eq!(ch.site.as_deref(), Some("神手谷"));
        assert_eq!(ch.world.as_deref(), Some("人界"));
        assert_eq!(ch.temporal_type.as_deref(), Some("JUMP_FORWARD"));
        assert_eq!(ch.time_offset_days, Some(1095));
        assert_eq!(ch.movement_type.as_deref(), Some("LOCAL_MOVE"));
        assert_eq!(ch.raw_text_summary.as_deref(), Some("韩立拾得掌天瓶"));

        assert_eq!(bundle.entities.len(), 2);
        let hanli = bundle.entities.iter().find(|e| e.name == "韩立").unwrap();
        assert_eq!(hanli.entity_type, "CHARACTER");
        assert_eq!(hanli.first_appeared_chapter_index, Some(0));
        assert!(bundle.entities.iter().any(|e| e.name == "掌天瓶" && e.entity_type == "ITEM"));
        assert_eq!(bundle.events.len(), 2);

        let books = db.list_books().unwrap();
        assert_eq!(books.len(), 1);
        assert_eq!(books[0].extracted_chapters, 1);
    }

    #[test]
    fn cascade_delete_removes_children() {
        let db = Db::open_in_memory().unwrap();
        let book_id = db.insert_book("B", None, "MOBI", 1).unwrap();
        let ch_id = db.insert_chapter(book_id, 0, "c0", "h", None).unwrap();
        db.persist_extraction(book_id, ch_id, &sample_extraction()).unwrap();

        db.delete_book(book_id).unwrap();
        assert!(db.list_books().unwrap().is_empty());
        assert!(db.get_entities(book_id).unwrap().is_empty());
        assert!(db.get_events(book_id).unwrap().is_empty());
        assert!(db.get_chapters(book_id).unwrap().is_empty());
    }

    #[test]
    fn insert_chapter_is_idempotent() {
        let db = Db::open_in_memory().unwrap();
        let book_id = db.insert_book("B", None, "EPUB", 1).unwrap();
        let a = db.insert_chapter(book_id, 0, "t", "h1", None).unwrap();
        let b = db.insert_chapter(book_id, 0, "t-again", "h2", None).unwrap();
        assert_eq!(a, b);
        assert_eq!(db.get_chapters(book_id).unwrap().len(), 1);
    }
}
