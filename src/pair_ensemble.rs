//! AIアンサンブルペアリングモジュール
//!
//! コンタクトシートを使った1コール1問×3走査のアンサンブルでペアリングを行う。

use crate::contactsheet::ContactSheet;
use crate::engine;
use crate::error::Result;

/// 1つのBefore画像について、3回走査のアンサンブルでAfter番号を決定
pub fn ensemble_pair_query(
    before_sheet: &ContactSheet,
    after_sheet: &ContactSheet,
    query: &str,
    before_max: u32,
    after_max: u32,
    verbose: bool,
) -> Result<(u32, f64)> {
    let (after_number, confidence) = engine::run_pair_ensemble(
        &before_sheet.image_path,
        &after_sheet.image_path,
        query,
        before_max,
        after_max,
    )?;

    if verbose {
        eprintln!("  [{}] Engine result: A{:02} confidence={:.2}", query, after_number, confidence);
    }

    Ok((after_number, confidence))
}
