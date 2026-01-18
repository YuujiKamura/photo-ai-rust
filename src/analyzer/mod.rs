mod claude_cli;
mod types;
pub mod cache;

pub use types::AnalysisResult;
pub use cache::{CacheFile, filter_cached_images};
pub use claude_cli::{RawImageData, Step2Result, analyze_batch_with_master};

use crate::error::Result;
use crate::scanner::ImageInfo;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::Path;

pub async fn analyze_images(
    images: &[ImageInfo],
    batch_size: usize,
    verbose: bool,
) -> Result<Vec<AnalysisResult>> {
    let mut results = Vec::new();
    let total_batches = images.len().div_ceil(batch_size);

    // プログレスバーの設定（推定残り時間・処理速度表示）
    let pb = ProgressBar::new(total_batches as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} バッチ | 残り {eta} | {msg}")
            .unwrap()
            .progress_chars("=>-"),
    );
    pb.enable_steady_tick(std::time::Duration::from_millis(100));

    // バッチに分割
    for (batch_idx, batch) in images.chunks(batch_size).enumerate() {
        pb.set_message(format!("{}枚処理中", batch.len()));

        if verbose {
            pb.suspend(|| {
                println!("  バッチ {}: {}枚", batch_idx + 1, batch.len());
            });
        }

        let batch_results = claude_cli::analyze_batch(batch, verbose).await?;
        results.extend(batch_results);

        pb.inc(1);
    }

    pb.finish_with_message("完了");

    Ok(results)
}

/// キャッシュを使用して画像を解析
///
/// - キャッシュにある画像はスキップ
/// - 新規画像のみ解析してキャッシュに追加
pub async fn analyze_images_with_cache(
    images: &[ImageInfo],
    folder: &Path,
    batch_size: usize,
    verbose: bool,
) -> Result<Vec<AnalysisResult>> {
    // キャッシュを読み込み
    let mut cache = CacheFile::load(folder);
    let initial_cache_size = cache.len();

    // キャッシュ済みと未キャッシュを分離
    let (mut cached_results, uncached_images) = filter_cached_images(images, &cache);

    if verbose {
        println!("  キャッシュヒット: {}枚", cached_results.len());
        println!("  未キャッシュ: {}枚", uncached_images.len());
    }

    // 未キャッシュ画像があれば解析
    if !uncached_images.is_empty() {
        let images_to_analyze: Vec<ImageInfo> = uncached_images.iter().map(|(img, _)| img.clone()).collect();
        let hashes: Vec<String> = uncached_images.iter().map(|(_, hash)| hash.clone()).collect();

        let new_results = analyze_images(&images_to_analyze, batch_size, verbose).await?;

        // 新規結果をキャッシュに追加
        for (i, result) in new_results.iter().enumerate() {
            if i < hashes.len() && !hashes[i].is_empty() {
                let img = &images_to_analyze[i];
                let file_size = img.path.metadata().map(|m| m.len()).unwrap_or(0);
                cache.insert(hashes[i].clone(), img.file_name.clone(), file_size, result.clone());
            }
        }

        cached_results.extend(new_results);

        // キャッシュを保存
        if cache.len() > initial_cache_size {
            cache.save(folder)?;
            if verbose {
                println!("  キャッシュ更新: {}件 → {}件", initial_cache_size, cache.len());
            }
        }
    }

    // ファイル名でソート（元の順序を維持）
    let file_order: std::collections::HashMap<&str, usize> = images
        .iter()
        .enumerate()
        .map(|(i, img)| (img.file_name.as_str(), i))
        .collect();

    cached_results.sort_by_key(|r| file_order.get(r.file_name.as_str()).copied().unwrap_or(usize::MAX));

    Ok(cached_results)
}

/// マスタを使用した2段階解析
///
/// - Step1: 画像認識（OCR、数値、シーン説明）
/// - Step2: 階層マスタとの照合で分類
pub async fn analyze_images_with_master(
    images: &[ImageInfo],
    master: &photo_ai_common::HierarchyMaster,
    batch_size: usize,
    verbose: bool,
) -> Result<Vec<AnalysisResult>> {
    let mut results = Vec::new();
    let total_batches = images.len().div_ceil(batch_size);

    // プログレスバーの設定
    let pb = ProgressBar::new(total_batches as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} バッチ | 残り {eta} | {msg}")
            .unwrap()
            .progress_chars("=>-"),
    );
    pb.enable_steady_tick(std::time::Duration::from_millis(100));

    // バッチに分割して2段階解析
    for (batch_idx, batch) in images.chunks(batch_size).enumerate() {
        pb.set_message(format!("{}枚 2段階解析中", batch.len()));

        if verbose {
            pb.suspend(|| {
                println!("  バッチ {}: {}枚 (2段階解析)", batch_idx + 1, batch.len());
            });
        }

        let batch_results = claude_cli::analyze_batch_with_master(batch, master, verbose).await?;
        results.extend(batch_results);

        pb.inc(1);
    }

    pb.finish_with_message("2段階解析完了");

    Ok(results)
}
