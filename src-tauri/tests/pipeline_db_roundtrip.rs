//! 集成测试：通过库的公开 API，验证「LLM JSON 解析 → SQLite 持久化 → 时空聚合查询」全链路。
//!
//! 运行：`cargo test`（或 `npm run test:rust`）。

use chronospace_reader_lib::db::Db;
use chronospace_reader_lib::pipeline::parse_extraction;

const MODEL_OUTPUT: &str = r#"
{
  "chapter_index": 0,
  "chapter_title": "七玄深谷",
  "narrative_perspective": "third_person",
  "temporal": { "type": "JUMP_FORWARD", "raw_marker": "三年后", "relative_delta_days": 1095 },
  "spatial": { "world": "人界", "region": "越国", "site": "神手谷", "movement_type": "STAY" },
  "characters": [ { "name": "韩立", "action": "FIRST_APPEARANCE", "role": "主角" } ],
  "items_and_skills": [ { "name": "掌天瓶", "category": "ITEM", "future_hook_flag": true } ],
  "plot_summary": "韩立得到掌天瓶"
}
"#;

#[test]
fn parse_then_persist_then_query() {
    let db = Db::open_in_memory().expect("内存数据库");
    let book_id = db
        .insert_book("集成测试", Some("作者"), "EPUB", 1, None, None)
        .unwrap();
    let chapter_id = db
        .insert_chapter(book_id, 0, "七玄深谷", "hash", None)
        .unwrap();

    let (extraction, repaired) = parse_extraction(MODEL_OUTPUT).expect("解析模型输出");
    assert!(!repaired, "合法 JSON 不应触发修复");

    db.persist_extraction(book_id, chapter_id, &extraction)
        .unwrap();

    let bundle = db.timeline_bundle(book_id).unwrap();
    assert_eq!(bundle.book.title, "集成测试");
    assert_eq!(bundle.chapters.len(), 1);
    assert_eq!(bundle.chapters[0].site.as_deref(), Some("神手谷"));
    assert_eq!(bundle.chapters[0].time_offset_days, Some(1095));
    assert_eq!(bundle.entities.len(), 2);
    assert_eq!(bundle.events.len(), 2);
}
