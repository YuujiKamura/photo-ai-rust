//! パイプライン統合スモークテスト
//!
//! domain / port / TypeState が協調して動作することを end-to-end で証明する。
//!
//! - AI 呼び出しは MockOracle で固定レスポンス
//! - マスタは InMemoryMasterRepository で in-memory 組み立て
//! - PDF/Excel 出力は MockRenderer で固定バイト列
//!
//! これにより外部依存なしに解析パイプライン全体のコントラクトをテストできる。

use photo_ai_common::domain::{Photo, PhotoCategory, Raw, Station};
use photo_ai_common::hierarchy::HierarchyRow;
use photo_ai_common::port::master::{InMemoryMasterRepository, MasterRepository};
use photo_ai_common::port::report::{MockRenderer, OutputFormat, ReportRenderer};
use photo_ai_common::port::vision::{ImageRef, MockOracle, VisionOracle};
use photo_ai_common::types::{AnalysisResult, RawImageData};

fn row(work_type: &str, variety: &str, subphase: &str, remarks: &str) -> HierarchyRow {
    HierarchyRow {
        photo_division: "直接工事費".to_string(),
        photo_type: "施工状況写真".to_string(),
        work_type: work_type.to_string(),
        variety: variety.to_string(),
        subphase: subphase.to_string(),
        remarks: remarks.to_string(),
        search_patterns: String::new(),
    }
}

#[tokio::test]
async fn full_pipeline_produces_rendered_output_via_mocks() {
    // === 1. Arrange: 3つのポート実装を Mock で用意 ===

    let oracle = MockOracle::new().with_response(
        "IMG_001.JPG",
        RawImageData {
            file_name: "IMG_001.JPG".to_string(),
            has_board: true,
            detected_text: "工種:舗装工\n場所:No.6 R\n舗設状況".to_string(),
            measurements: String::new(),
            scene_description: "フィニッシャーで舗設中".to_string(),
            photo_category: String::new(),
        },
    );

    let master_repo = InMemoryMasterRepository::new().with_work_type(
        "舗装工",
        vec![
            row("舗装工", "舗装打換え工", "表層工", "舗設状況"),
            row("舗装工", "舗装打換え工", "表層工", "初期転圧状況"),
        ],
    );

    let fixed_pdf_bytes = vec![0x25, 0x50, 0x44, 0x46, 0xAB, 0xCD]; // "%PDF" + noise
    let renderer = MockRenderer::new(OutputFormat::Pdf, fixed_pdf_bytes.clone());

    // === 2. Act: Oracle → Photo<Raw> ===

    let images = [ImageRef::new("IMG_001.JPG", "/tmp/photos/IMG_001.JPG")
        .with_date("2026-02-11 10:00:00")];
    let raw_batch = oracle
        .analyze_batch(&images, "step1 prompt")
        .await
        .expect("oracle analyze_batch");

    assert_eq!(raw_batch.len(), 1, "oracle must return one result per image");
    let raw = raw_batch.into_iter().next().unwrap();

    let photo_raw = Photo::<Raw>::from_raw_data(
        raw,
        "/tmp/photos/IMG_001.JPG".to_string(),
        "2026-02-11 10:00:00".to_string(),
    );
    assert_eq!(photo_raw.file_name(), "IMG_001.JPG");
    assert!(photo_raw.has_board());

    // === 3. Photo<Raw> → Classified ===
    // 実際のコードでは detected_text から work_type を抽出するが、
    // ここではドメイン決定を手動で再現（スモークテストの目的は型の連結確認）

    let photo_classified = photo_raw.classify(
        PhotoCategory::Construction,
        Some("舗装工".to_string()),
        "フィニッシャーで舗設中".to_string(),
    );
    assert_eq!(photo_classified.photo_category(), PhotoCategory::Construction);

    // === 4. マスタ照合: MasterRepository を使う ===

    let master = master_repo
        .load_by_work_type("舗装工")
        .expect("master load");
    assert_eq!(master.rows().len(), 2);

    // 実際は detected_text → マスタ検索パターン照合だが、ここは "舗設状況" に手動でマッチさせる
    let hit = master
        .rows()
        .iter()
        .find(|r| r.remarks == "舗設状況")
        .expect("expected master hit for 舗設状況");

    let photo_matched = photo_classified.with_master_match(
        hit.variety.clone(),
        hit.subphase.clone(),
        hit.remarks.clone(),
    );
    assert_eq!(photo_matched.variety(), "舗装打換え工");
    assert_eq!(photo_matched.subphase(), "表層工");
    assert_eq!(photo_matched.remarks(), "舗設状況");

    // === 5. 正規化: Station enum + measurements ===

    let photo_normalized = photo_matched.normalize(Station::parse("No.6 R"), None);
    let station = photo_normalized.station();
    assert!(station.is_post(), "expected Post, got {:?}", station);

    // === 6. 出力可能状態に遷移 ===

    let photo_exportable = photo_normalized.finalize();
    assert_eq!(photo_exportable.station_str(), "No.6 R");

    // === 7. ReportRenderer でバイト列生成 ===

    let analysis: AnalysisResult = photo_exportable.into();
    let rendered = renderer
        .render(&[analysis], &(|_| None))
        .expect("renderer produces bytes");
    assert_eq!(
        rendered, fixed_pdf_bytes,
        "MockRenderer must return configured fixed bytes"
    );
}

#[tokio::test]
async fn pipeline_without_master_hit_takes_fast_path() {
    // MasterRepository にマッチするエントリがないケースの流れ。
    // マスタヒットなしでも into_matched_without_master で後続 phase に進める。

    let oracle = MockOracle::new().with_default(RawImageData {
        file_name: "_".to_string(),
        has_board: false,
        detected_text: String::new(),
        measurements: String::new(),
        scene_description: "重機の写真".to_string(),
        photo_category: String::new(),
    });

    let images = [ImageRef::new("IMG_002.JPG", "/tmp/IMG_002.JPG")];
    let raw = oracle
        .analyze_batch(&images, "")
        .await
        .unwrap()
        .into_iter()
        .next()
        .unwrap();

    let photo = Photo::<Raw>::from_raw_data(raw, "/tmp/IMG_002.JPG".into(), "2026-02-11".into())
        .classify(PhotoCategory::Other, Some("舗装工".into()), String::new())
        .into_matched_without_master()
        .normalize(Station::Empty, None)
        .finalize();

    assert_eq!(photo.remarks(), ""); // マスタヒットなし
    assert_eq!(photo.station_str(), "");
    assert_eq!(photo.photo_category(), PhotoCategory::Other);
}

#[tokio::test]
async fn mock_oracle_log_captures_pipeline_calls() {
    let oracle = MockOracle::new().with_default(RawImageData::default());
    let images = [
        ImageRef::new("a.jpg", "/tmp/a.jpg"),
        ImageRef::new("b.jpg", "/tmp/b.jpg"),
    ];

    oracle.analyze_batch(&images, "first").await.unwrap();
    oracle.analyze_batch(&images, "second").await.unwrap();

    let log = oracle.snapshot_log();
    assert_eq!(log.call_count, 2);
    assert_eq!(log.last_prompt.as_deref(), Some("second"));
    assert_eq!(log.last_filenames, vec!["a.jpg", "b.jpg"]);
}

#[tokio::test]
async fn batch_renders_byte_per_photo_via_count_echo() {
    // MockRenderer::count_echo は結果件数ぶんの 'A' を返す → バッチ処理の正当性検証用

    let renderer = MockRenderer::count_echo(OutputFormat::Excel);
    let photos = vec![AnalysisResult::default(); 5];
    let out = renderer.render(&photos, &(|_| None)).unwrap();
    assert_eq!(out, b"AAAAA");
    assert_eq!(renderer.output_format(), OutputFormat::Excel);
}

#[test]
fn master_repo_roundtrips_all_work_types_via_load_all() {
    let repo = InMemoryMasterRepository::new()
        .with_work_type("舗装工", vec![row("舗装工", "舗装打換え工", "表層工", "舗設状況")])
        .with_work_type(
            "区画線工",
            vec![row("区画線工", "実線", "設置工", "設置状況")],
        );

    let names = repo.list_work_types().unwrap();
    assert_eq!(names, vec!["区画線工".to_string(), "舗装工".to_string()]);

    let merged = repo.load_all().unwrap();
    let work_types: std::collections::HashSet<&str> = merged
        .rows()
        .iter()
        .map(|r| r.work_type.as_str())
        .collect();
    assert!(work_types.contains("舗装工"));
    assert!(work_types.contains("区画線工"));
}
