mod claude_cli;
mod types;

pub use types::AnalysisResult;

use crate::error::Result;
use crate::scanner::ImageInfo;

pub async fn analyze_images(
    images: &[ImageInfo],
    batch_size: usize,
    verbose: bool,
) -> Result<Vec<AnalysisResult>> {
    let mut results = Vec::new();

    // バッチに分割
    for (batch_idx, batch) in images.chunks(batch_size).enumerate() {
        if verbose {
            println!("  バッチ {}: {}枚", batch_idx + 1, batch.len());
        }

        let batch_results = claude_cli::analyze_batch(batch, verbose).await?;
        results.extend(batch_results);
    }

    Ok(results)
}
