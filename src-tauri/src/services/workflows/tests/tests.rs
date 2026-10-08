use super::archive::*;
use super::chain::*;
use super::document::*;
use super::group_files::*;
use super::scheduler::*;
use super::summarizer::*;
use crate::services::db::Database;
use std::path::Path;

// --- Archive Tests ---

fn sample_zip() -> Vec<u8> {
    crate::services::diagnostics::build_zip(&[
        ("hello.txt".to_string(), b"hello world".to_vec()),
        (
            "nested/dir/中文.xml".to_string(),
            b"<a>\xe4\xb8\xad\xe6\x96\x87</a>".to_vec(),
        ),
        ("empty.txt".to_string(), Vec::new()),
    ])
}

#[test]
fn archive_reads_back_what_was_written() {
    let zip = sample_zip();
    let names = entry_names(&zip).unwrap();
    assert_eq!(names.len(), 3);
    assert!(names.contains(&"hello.txt".to_string()));

    let hello = read_entry(&zip, "hello.txt").unwrap().unwrap();
    assert_eq!(hello, b"hello world");

    let nested = read_entry(&zip, "nested/dir/中文.xml").unwrap().unwrap();
    assert!(String::from_utf8_lossy(&nested).contains("中文"));

    let empty = read_entry(&zip, "empty.txt").unwrap().unwrap();
    assert!(empty.is_empty());
}

#[test]
fn archive_lookup_is_case_insensitive() {
    let zip = sample_zip();
    assert!(read_entry(&zip, "HELLO.TXT").unwrap().is_some());
}

#[test]
fn archive_missing_entry_is_none() {
    let zip = sample_zip();
    assert!(read_entry(&zip, "absent.bin").unwrap().is_none());
}

#[test]
fn archive_rejects_non_zip_input() {
    assert!(list_entries(b"this is definitely not a zip archive").is_err());
    assert!(list_entries(&[]).is_err());
    assert!(list_entries(b"PK").is_err());
}

// --- Document Tests ---

#[test]
fn document_paragraphs_become_newlines() {
    let xml = "<w:p><w:r><w:t>第一段</w:t></w:r></w:p><w:p><w:r><w:t>第二段</w:t></w:r></w:p>";
    assert_eq!(xml_to_text(xml), "第一段\n第二段");
}

#[test]
fn document_entities_are_decoded() {
    assert_eq!(decode_entities("a&amp;b"), "a&b");
    assert_eq!(decode_entities("&lt;tag&gt;"), "<tag>");
    assert_eq!(decode_entities("&quot;q&quot;"), "\"q\"");
}

#[test]
fn document_docx_extractor_rejects_garbage() {
    assert!(extract_docx(b"not a zip at all").is_err());
}

#[test]
fn document_pdf_extractor_reports_failure() {
    let err = extract_pdf(b"%PDF-1.4\nno text operators here").unwrap_err();
    assert!(err.contains("PDF"), "got: {err}");
}

// --- Group Files Tests ---

#[test]
fn sanitize_strips_path_separators() {
    assert_eq!(sanitize_file_name("../../etc/passwd"), "_.._etc_passwd");
    assert_eq!(
        sanitize_file_name("..\\..\\windows\\system32"),
        "_.._windows_system32"
    );
    assert_eq!(sanitize_file_name("a/b/c.txt"), "a_b_c.txt");
}

#[test]
fn sanitize_strips_leading_dots() {
    assert_eq!(sanitize_file_name(".hidden"), "hidden");
    assert_eq!(sanitize_file_name("re:port?.txt"), "re_port_.txt");
}

#[test]
fn extension_detection_is_case_insensitive() {
    assert_eq!(extension_of("Report.DOCX"), "docx");
    assert_eq!(extension_of("no_extension"), "");
    assert_eq!(extension_of("archive.tar.gz"), "gz");
}

#[test]
fn missing_file_reports_clearly() {
    let err = extract_text(Path::new("B:/definitely/not/here.txt")).unwrap_err();
    assert!(err.contains("文件不存在"), "got: {err}");
}

// --- Chain Tests ---

#[test]
fn chain_unknown_until_something_reports() {
    let reports = snapshot();
    assert_eq!(reports.len(), Link::all().len());
}

#[test]
fn chain_failure_is_recorded() {
    record_error(Link::OneBotWs, "connection refused");
    let report = snapshot()
        .into_iter()
        .find(|r| r.link == Link::OneBotWs)
        .expect("link present");
    assert_eq!(report.health, Health::Failed);
    assert_eq!(report.detail, "connection refused");
}

// --- Summarizer Tests ---

#[test]
fn noise_filter_drops_emoji() {
    assert!(is_noise(""));
    assert!(is_noise("   "));
    assert!(is_noise("。。。"));
    assert!(is_noise("🎉🎉🎉"));
    assert!(!is_noise("好的"));
    assert!(!is_noise("这个方案我同意"));
}

#[test]
fn flood_dedup_keeps_at_most_two() {
    let msg = |s: &str| ("张三".to_string(), s.to_string(), "0".to_string());
    let input = vec![msg("刷屏"), msg("刷屏"), msg("刷屏"), msg("换一句")];
    let out = dedupe_flood(&input);
    assert_eq!(out.len(), 3);
    assert_eq!(out[2].1, "换一句");
}

// --- Scheduler Tests ---

fn temp_db(tag: &str) -> std::sync::Arc<Database> {
    let dir = std::env::temp_dir().join("eazyqq_scheduler_tests");
    crate::services::logging::ensure_dir(&dir);
    let path = dir.join(format!("{}_{}.db", tag, std::process::id()));
    let _ = std::fs::remove_file(&path);
    std::sync::Arc::new(Database::init(&path).expect("temp db"))
}

#[test]
fn interval_type_maps_to_hours() {
    for (kind, expected) in [
        ("1h", 1),
        ("2h", 2),
        ("4h", 4),
        ("6h", 6),
        ("12h", 12),
        ("24h", 24),
    ] {
        let v = serde_json::json!({ "intervalType": kind });
        assert_eq!(resolve_interval_hours(&v), expected, "intervalType {kind}");
    }
}

#[test]
fn settings_fall_back_to_defaults() {
    let db = temp_db("no_config");
    let s = read_settings(&db);
    assert!(s.enabled);
    assert_eq!(s.sliding_window_hours, 6);
}
