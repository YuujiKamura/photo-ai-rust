//! キャッシュ機能テスト
//!
//! 解析結果キャッシュの動作を検証

use photo_ai_rust::analyzer::cache::{CacheFile, filter_cached_images};
use photo_ai_rust::analyzer::AnalysisResult;
use photo_ai_rust::scanner::ImageInfo;
use tempfile::tempdir;

/// 空のキャッシュファイル
#[test]
fn test_cache_file_empty() {
    let dir = tempdir().expect("Failed to create temp dir");
    let cache = CacheFile::load(dir.path());

    assert_eq!(cache.len(), 0);
    assert!(cache.is_empty());
}

/// キャッシュの保存と読み込み
#[test]
fn test_cache_save_and_load() {
    let dir = tempdir().expect("Failed to create temp dir");

    // キャッシュを作成して保存
    let mut cache = CacheFile::load(dir.path());
    let result = AnalysisResult {
        file_name: "test.jpg".to_string(),
        work_type: "舗装工".to_string(),
        ..Default::default()
    };

    cache.insert(
        "abc123".to_string(),
        "test.jpg".to_string(),
        1024,
        result.clone(),
    );

    cache.save(dir.path()).expect("キャッシュ保存失敗");

    // 再読み込み
    let loaded = CacheFile::load(dir.path());
    assert_eq!(loaded.len(), 1);

    let cached = loaded.get("abc123").expect("キャッシュが見つからない");
    assert_eq!(cached.file_name, "test.jpg");
    assert_eq!(cached.work_type, "舗装工");
}

/// キャッシュヒット判定
#[test]
fn test_cache_hit() {
    let dir = tempdir().expect("Failed to create temp dir");

    let mut cache = CacheFile::load(dir.path());
    let result = AnalysisResult {
        file_name: "cached.jpg".to_string(),
        work_type: "区画線工".to_string(),
        ..Default::default()
    };

    // MD5ハッシュをシミュレート
    let hash = "d41d8cd98f00b204e9800998ecf8427e";
    cache.insert(
        hash.to_string(),
        "cached.jpg".to_string(),
        2048,
        result,
    );

    // キャッシュにある → ヒット
    assert!(cache.get(hash).is_some());

    // キャッシュにない → ミス
    assert!(cache.get("nonexistent_hash").is_none());
}

/// キャッシュの複数エントリ
#[test]
fn test_cache_multiple_entries() {
    let dir = tempdir().expect("Failed to create temp dir");

    let mut cache = CacheFile::load(dir.path());

    for i in 1..=5 {
        let result = AnalysisResult {
            file_name: format!("photo_{}.jpg", i),
            work_type: format!("工種{}", i),
            ..Default::default()
        };

        cache.insert(
            format!("hash_{}", i),
            format!("photo_{}.jpg", i),
            1000 * i as u64,
            result,
        );
    }

    assert_eq!(cache.len(), 5);

    // 各エントリを検証
    for i in 1..=5 {
        let cached = cache.get(&format!("hash_{}", i)).expect("キャッシュが見つからない");
        assert_eq!(cached.file_name, format!("photo_{}.jpg", i));
    }
}

/// filter_cached_imagesのテスト
#[test]
fn test_filter_cached_images() {
    let dir = tempdir().expect("Failed to create temp dir");

    // テスト用の画像ファイルを作成
    let img1_path = dir.path().join("img1.jpg");
    let img2_path = dir.path().join("img2.jpg");
    std::fs::write(&img1_path, b"fake image 1").unwrap();
    std::fs::write(&img2_path, b"fake image 2").unwrap();

    let images = vec![
        ImageInfo {
            file_name: "img1.jpg".to_string(),
            path: img1_path.clone(),
            date: None,
        },
        ImageInfo {
            file_name: "img2.jpg".to_string(),
            path: img2_path.clone(),
            date: Some("2026-01-18".to_string()),
        },
    ];

    // 空のキャッシュ → 全て未キャッシュ
    let cache = CacheFile::load(dir.path());
    let (cached, uncached) = filter_cached_images(&images, &cache);

    assert!(cached.is_empty());
    assert_eq!(uncached.len(), 2);
}

/// キャッシュの上書き
#[test]
fn test_cache_overwrite() {
    let dir = tempdir().expect("Failed to create temp dir");

    let mut cache = CacheFile::load(dir.path());
    let hash = "same_hash";

    // 最初のエントリ
    let result1 = AnalysisResult {
        file_name: "test.jpg".to_string(),
        work_type: "最初の工種".to_string(),
        ..Default::default()
    };
    cache.insert(hash.to_string(), "test.jpg".to_string(), 1000, result1);

    // 上書き
    let result2 = AnalysisResult {
        file_name: "test.jpg".to_string(),
        work_type: "更新後の工種".to_string(),
        ..Default::default()
    };
    cache.insert(hash.to_string(), "test.jpg".to_string(), 1000, result2);

    // 最新の値が取得される
    let cached = cache.get(hash).expect("キャッシュが見つからない");
    assert_eq!(cached.work_type, "更新後の工種");
    assert_eq!(cache.len(), 1); // エントリ数は変わらない
}

/// キャッシュファイルが破損している場合
#[test]
fn test_cache_corrupted_file() {
    let dir = tempdir().expect("Failed to create temp dir");
    let cache_path = dir.path().join(".step1-cache.json");

    // 不正なJSONを書き込む
    std::fs::write(&cache_path, "{ invalid json }").unwrap();

    // 破損したキャッシュは空として扱われる
    let cache = CacheFile::load(dir.path());
    assert!(cache.is_empty());
}

/// キャッシュのバージョン互換性
#[test]
fn test_cache_version_compatibility() {
    let dir = tempdir().expect("Failed to create temp dir");

    // 現在のバージョンでキャッシュを作成
    let mut cache = CacheFile::load(dir.path());
    let result = AnalysisResult {
        file_name: "version_test.jpg".to_string(),
        ..Default::default()
    };
    cache.insert("hash".to_string(), "version_test.jpg".to_string(), 100, result);
    cache.save(dir.path()).expect("保存失敗");

    // 再読み込みでバージョンが正しく処理される
    let loaded = CacheFile::load(dir.path());
    assert_eq!(loaded.len(), 1);
}
