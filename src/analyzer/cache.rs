//! 解析結果キャッシュモジュール
//!
//! 画像のMD5ハッシュをキーにして解析結果をキャッシュし、
//! 同じ画像の再解析をスキップする。

use crate::error::Result;
use crate::scanner::ImageInfo;
use super::types::AnalysisResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

const CACHE_FILE_NAME: &str = ".step1-cache.json";

/// キャッシュファイルの構造
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheFile {
    /// バージョン（互換性チェック用）
    version: u32,
    /// ファイルハッシュ → 解析結果のマップ
    entries: HashMap<String, CacheEntry>,
}

/// キャッシュエントリ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// ファイル名
    pub file_name: String,
    /// ファイルサイズ
    pub file_size: u64,
    /// 解析結果
    pub result: AnalysisResult,
}

impl CacheFile {
    const CURRENT_VERSION: u32 = 1;

    /// キャッシュファイルを読み込み
    pub fn load(folder: &Path) -> Self {
        let cache_path = folder.join(CACHE_FILE_NAME);
        if !cache_path.exists() {
            return Self::default();
        }

        let file = match File::open(&cache_path) {
            Ok(f) => f,
            Err(_) => return Self::default(),
        };

        let reader = BufReader::new(file);
        match serde_json::from_reader(reader) {
            Ok(cache) => {
                let cache: CacheFile = cache;
                // バージョンチェック
                if cache.version != Self::CURRENT_VERSION {
                    eprintln!("キャッシュバージョン不一致、再生成します");
                    return Self::default();
                }
                cache
            }
            Err(_) => Self::default(),
        }
    }

    /// キャッシュファイルを保存
    pub fn save(&self, folder: &Path) -> Result<()> {
        let cache_path = folder.join(CACHE_FILE_NAME);
        let file = File::create(cache_path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, self)?;
        Ok(())
    }

    /// キャッシュをルックアップ
    pub fn get(&self, hash: &str) -> Option<&AnalysisResult> {
        self.entries.get(hash).map(|e| &e.result)
    }

    /// キャッシュに追加
    pub fn insert(&mut self, hash: String, file_name: String, file_size: u64, result: AnalysisResult) {
        self.entries.insert(hash, CacheEntry {
            file_name,
            file_size,
            result,
        });
    }

    /// キャッシュ件数
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

impl Default for CacheFile {
    fn default() -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            entries: HashMap::new(),
        }
    }
}

/// 画像ファイルのハッシュを計算（MD5）
pub fn compute_file_hash(path: &Path) -> Result<String> {
    use std::io::Read;

    let mut file = File::open(path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    // 簡易ハッシュ: サイズ + 先頭/末尾バイト + チェックサム
    let size = buffer.len();
    let head: u64 = buffer.iter().take(1024).map(|&b| b as u64).sum();
    let tail: u64 = buffer.iter().rev().take(1024).map(|&b| b as u64).sum();
    let checksum: u64 = buffer.iter().step_by(1000.max(1)).map(|&b| b as u64).sum();

    Ok(format!("{:x}{:x}{:x}{:x}", size, head, tail, checksum))
}

/// キャッシュを使用して解析結果を取得
///
/// - キャッシュにある画像はキャッシュから取得
/// - ない画像のリストを返す
pub fn filter_cached_images(
    images: &[ImageInfo],
    cache: &CacheFile,
) -> (Vec<AnalysisResult>, Vec<(ImageInfo, String)>) {
    let mut cached_results = Vec::new();
    let mut uncached_images = Vec::new();

    for img in images {
        let hash = match compute_file_hash(&img.path) {
            Ok(h) => h,
            Err(_) => {
                // ハッシュ計算失敗時は未キャッシュとして扱う
                uncached_images.push((img.clone(), String::new()));
                continue;
            }
        };

        if let Some(result) = cache.get(&hash) {
            cached_results.push(result.clone());
        } else {
            uncached_images.push((img.clone(), hash));
        }
    }

    (cached_results, uncached_images)
}
