//! マスタリポジトリ port
//!
//! 工種マスタ CSV のロードを抽象化する。現状は CSV ファイルが唯一の実装だが、
//! テスト用の in-memory 実装や、将来の DB 実装を同一 trait 配下に置ける。

use crate::hierarchy::{HierarchyMaster, HierarchyRow};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

/// マスタロードのエラー
#[derive(Debug, thiserror::Error)]
pub enum MasterError {
    #[error("マスタが見つかりません: {0}")]
    NotFound(String),

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("CSV parse error: {0}")]
    Parse(String),
}

/// マスタリポジトリ port
///
/// 特定の工種のマスタだけロードしたい場合と、全工種をマージして扱いたい場合の
/// 2 ユースケースをサポートする。
pub trait MasterRepository: Send + Sync {
    /// 指定工種のマスタを共通マスタとマージしてロードする
    ///
    /// 工種が存在しない場合は `MasterError::NotFound` を返す。
    fn load_by_work_type(&self, work_type: &str) -> Result<HierarchyMaster, MasterError>;

    /// 全工種のマスタをマージしてロードする
    ///
    /// `master/by_work_type/*.csv` 相当＋共通マスタ。
    fn load_all(&self) -> Result<HierarchyMaster, MasterError>;

    /// 利用可能な工種名の一覧
    fn list_work_types(&self) -> Result<Vec<String>, MasterError>;
}

// ============================================================================
// CsvMasterRepository: ファイルシステムベースの実装
// ============================================================================

/// ディレクトリ配下の CSV 群を読み取るマスタリポジトリ
///
/// 期待するディレクトリ構造:
/// ```text
/// base_dir/
/// ├── by_work_type/
/// │   ├── 舗装工.csv
/// │   ├── 区画線工.csv
/// │   └── ...
/// └── common.csv  (オプション)
/// ```
pub struct CsvMasterRepository {
    base_dir: PathBuf,
}

impl CsvMasterRepository {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    fn by_work_type_dir(&self) -> PathBuf {
        self.base_dir.join("by_work_type")
    }

    fn common_csv_path(&self) -> PathBuf {
        self.base_dir.join("common.csv")
    }

    fn work_type_csv_path(&self, work_type: &str) -> PathBuf {
        self.by_work_type_dir().join(format!("{}.csv", work_type))
    }

    fn load_paths(&self, paths: &[PathBuf]) -> Result<HierarchyMaster, MasterError> {
        HierarchyMaster::from_csv_files(paths).map_err(|e| MasterError::Parse(e.to_string()))
    }

    fn existing(paths: Vec<PathBuf>) -> Vec<PathBuf> {
        paths.into_iter().filter(|p| p.exists()).collect()
    }
}

impl MasterRepository for CsvMasterRepository {
    fn load_by_work_type(&self, work_type: &str) -> Result<HierarchyMaster, MasterError> {
        let primary = self.work_type_csv_path(work_type);
        if !primary.exists() {
            return Err(MasterError::NotFound(format!(
                "{} (expected at {})",
                work_type,
                primary.display()
            )));
        }
        let paths = Self::existing(vec![primary, self.common_csv_path()]);
        self.load_paths(&paths)
    }

    fn load_all(&self) -> Result<HierarchyMaster, MasterError> {
        let dir = self.by_work_type_dir();
        if !dir.is_dir() {
            return Err(MasterError::NotFound(format!(
                "by_work_type directory not found at {}",
                dir.display()
            )));
        }
        let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)?
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("csv"))
            .collect();
        paths.sort();
        paths.push(self.common_csv_path());
        let paths = Self::existing(paths);
        if paths.is_empty() {
            return Err(MasterError::NotFound(format!(
                "no CSV files under {}",
                dir.display()
            )));
        }
        self.load_paths(&paths)
    }

    fn list_work_types(&self) -> Result<Vec<String>, MasterError> {
        let dir = self.by_work_type_dir();
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut names: Vec<String> = std::fs::read_dir(&dir)?
            .filter_map(|entry| entry.ok())
            .filter(|e| {
                e.path().extension().and_then(|s| s.to_str()) == Some("csv")
            })
            .filter_map(|e| {
                e.path()
                    .file_stem()
                    .and_then(|s| s.to_str().map(|s| s.to_string()))
            })
            .collect();
        names.sort();
        Ok(names)
    }
}

// ============================================================================
// InMemoryMasterRepository: テスト用
// ============================================================================

/// メモリ上にロウを保持する、テスト専用のマスタリポジトリ
///
/// temp dir + CSV 書き出しなしでユニットテストを書けるようにする。
#[derive(Default)]
pub struct InMemoryMasterRepository {
    by_work_type: HashMap<String, Vec<HierarchyRow>>,
    common: Vec<HierarchyRow>,
}

impl InMemoryMasterRepository {
    pub fn new() -> Self {
        Self::default()
    }

    /// 工種ごとの行を登録する（chain 用に self を返す）
    pub fn with_work_type(mut self, work_type: impl Into<String>, rows: Vec<HierarchyRow>) -> Self {
        self.by_work_type.insert(work_type.into(), rows);
        self
    }

    /// 共通マスタ行を登録する
    pub fn with_common(mut self, rows: Vec<HierarchyRow>) -> Self {
        self.common = rows;
        self
    }

    fn build_master(&self, rows: Vec<HierarchyRow>) -> HierarchyMaster {
        // CsvMasterRepository 経由と同じ形にするため from_csv_str を利用できないので
        // HierarchyMaster 内部の from_rows を呼ぶ代わりに CSV シリアライズ→パースで組み立てる
        // …のは冗長なので、HierarchyMaster に crate 内公開の from_rows API を期待する。
        // 現状の HierarchyMaster::from_rows は private のため、
        // ここでは from_csv_str で再構築する簡易実装を採用する。
        let csv = rows_to_csv(&rows);
        HierarchyMaster::from_csv_str(&csv).expect("internal CSV roundtrip should not fail")
    }
}

impl MasterRepository for InMemoryMasterRepository {
    fn load_by_work_type(&self, work_type: &str) -> Result<HierarchyMaster, MasterError> {
        let work_rows = self
            .by_work_type
            .get(work_type)
            .ok_or_else(|| MasterError::NotFound(work_type.to_string()))?;
        let mut merged = work_rows.clone();
        merged.extend(self.common.iter().cloned());
        Ok(self.build_master(merged))
    }

    fn load_all(&self) -> Result<HierarchyMaster, MasterError> {
        let mut merged: Vec<HierarchyRow> = Vec::new();
        let mut keys: Vec<&String> = self.by_work_type.keys().collect();
        keys.sort();
        for k in keys {
            merged.extend(self.by_work_type[k].clone());
        }
        merged.extend(self.common.iter().cloned());
        if merged.is_empty() {
            return Err(MasterError::NotFound("empty InMemoryMasterRepository".into()));
        }
        Ok(self.build_master(merged))
    }

    fn list_work_types(&self) -> Result<Vec<String>, MasterError> {
        let mut names: Vec<String> = self.by_work_type.keys().cloned().collect();
        names.sort();
        Ok(names)
    }
}

fn rows_to_csv(rows: &[HierarchyRow]) -> String {
    let mut out = String::from("費目,写真区分,工種,種別,細別,備考,検索パターン\n");
    for r in rows {
        out.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            escape_csv(&r.photo_division),
            escape_csv(&r.photo_type),
            escape_csv(&r.work_type),
            escape_csv(&r.variety),
            escape_csv(&r.subphase),
            escape_csv(&r.remarks),
            escape_csv(&r.search_patterns),
        ));
    }
    out
}

fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// `std::path::Path` のエクスポート互換のため（意味はない）
#[allow(dead_code)]
fn _ensure_path_reexport(_p: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_row(work_type: &str, variety: &str, remarks: &str) -> HierarchyRow {
        HierarchyRow {
            photo_division: "直接工事費".to_string(),
            photo_type: "施工状況写真".to_string(),
            work_type: work_type.to_string(),
            variety: variety.to_string(),
            subphase: "表層工".to_string(),
            remarks: remarks.to_string(),
            search_patterns: String::new(),
        }
    }

    // === InMemoryMasterRepository 契約 ===

    #[test]
    fn in_memory_load_by_work_type_merges_common() {
        let repo = InMemoryMasterRepository::new()
            .with_work_type(
                "舗装工",
                vec![sample_row("舗装工", "舗装打換え工", "舗設状況")],
            )
            .with_common(vec![sample_row("共通", "", "安全朝礼実施状況")]);

        let master = repo.load_by_work_type("舗装工").unwrap();
        assert_eq!(master.rows().len(), 2);
    }

    #[test]
    fn in_memory_load_by_work_type_not_found_errors() {
        let repo = InMemoryMasterRepository::new();
        let err = repo.load_by_work_type("存在しない工").unwrap_err();
        assert!(matches!(err, MasterError::NotFound(_)));
    }

    #[test]
    fn in_memory_load_all_merges_everything() {
        let repo = InMemoryMasterRepository::new()
            .with_work_type("舗装工", vec![sample_row("舗装工", "表層工", "A")])
            .with_work_type("区画線工", vec![sample_row("区画線工", "実線", "B")])
            .with_common(vec![sample_row("共通", "", "C")]);

        let master = repo.load_all().unwrap();
        assert_eq!(master.rows().len(), 3);
    }

    #[test]
    fn in_memory_list_work_types_is_sorted() {
        let repo = InMemoryMasterRepository::new()
            .with_work_type("区画線工", vec![])
            .with_work_type("舗装工", vec![])
            .with_work_type("構造物撤去工", vec![]);
        let names = repo.list_work_types().unwrap();
        // ソートは文字列順なので Rust の str Ord 基準
        assert_eq!(names.len(), 3);
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn in_memory_load_all_empty_errors() {
        let repo = InMemoryMasterRepository::new();
        assert!(matches!(
            repo.load_all().unwrap_err(),
            MasterError::NotFound(_)
        ));
    }

    // === CsvMasterRepository 契約（tempfile 使用） ===

    #[test]
    fn csv_repo_load_by_work_type_reads_expected_file() {
        let tmp = tempdir_with_sample_csv();
        let repo = CsvMasterRepository::new(tmp.path());
        let master = repo.load_by_work_type("舗装工").unwrap();
        assert!(master.rows().iter().any(|r| r.work_type == "舗装工"));
    }

    #[test]
    fn csv_repo_not_found_errors() {
        let tmp = tempdir_with_sample_csv();
        let repo = CsvMasterRepository::new(tmp.path());
        assert!(matches!(
            repo.load_by_work_type("存在しない工").unwrap_err(),
            MasterError::NotFound(_)
        ));
    }

    #[test]
    fn csv_repo_list_work_types_returns_csv_stems() {
        let tmp = tempdir_with_sample_csv();
        let repo = CsvMasterRepository::new(tmp.path());
        let names = repo.list_work_types().unwrap();
        assert!(names.contains(&"舗装工".to_string()));
    }

    #[test]
    fn csv_repo_load_all_merges_all_csv() {
        let tmp = tempdir_with_sample_csv();
        let repo = CsvMasterRepository::new(tmp.path());
        let master = repo.load_all().unwrap();
        // 舗装工 + 区画線工 + common のいずれかが入っていること
        let work_types: std::collections::HashSet<&str> =
            master.rows().iter().map(|r| r.work_type.as_str()).collect();
        assert!(work_types.contains("舗装工"));
    }

    fn tempdir_with_sample_csv() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().expect("tempdir");
        let by_wt = tmp.path().join("by_work_type");
        std::fs::create_dir(&by_wt).unwrap();
        std::fs::write(
            by_wt.join("舗装工.csv"),
            "費目,写真区分,工種,種別,細別,備考,検索パターン\n\
             直接工事費,施工状況写真,舗装工,舗装打換え工,表層工,舗設状況,\n",
        )
        .unwrap();
        std::fs::write(
            by_wt.join("区画線工.csv"),
            "費目,写真区分,工種,種別,細別,備考,検索パターン\n\
             直接工事費,施工状況写真,区画線工,実線,設置工,設置状況,\n",
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("common.csv"),
            "費目,写真区分,工種,種別,細別,備考,検索パターン\n\
             共通仮設費,安全管理写真,,,,安全朝礼実施状況,\n",
        )
        .unwrap();
        tmp
    }
}
