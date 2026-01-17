mod claude_cli;
mod types;

pub use types::AnalysisResult;

use crate::error::Result;
use crate::scanner::ImageInfo;
use indicatif::{ProgressBar, ProgressStyle};

pub async fn analyze_images(
    images: &[ImageInfo],
    batch_size: usize,
    verbose: bool,
) -> Result<Vec<AnalysisResult>> {
    let mut results = Vec::new();
    let total_batches = (images.len() + batch_size - 1) / batch_size;

    // プログレスバーの設定
    let pb = ProgressBar::new(total_batches as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} バッチ ({msg})")
            .unwrap()
            .progress_chars("=>-"),
    );

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
