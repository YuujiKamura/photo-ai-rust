//! `Photo<Phase>` — 解析パイプラインの段階を型で表現する
//!
//! TypeState パターンで、Raw → Classified → Matched → Normalized → Exportable
//! の線形遷移を型で保証する。不正状態（未分類の写真を PDF 出力する、マスタ照合前に
//! station を参照する等）がコンパイル時に排除される。
//!
//! 既存の `AnalysisResult` はそのまま残し、`Photo<Phase>` は opt-in で使える
//! 型安全ラッパーとして並置する。
//!
//! ## 使用例
//!
//! ```
//! use photo_ai_common::domain::photo::{Photo, Raw};
//! use photo_ai_common::domain::PhotoCategory;
//!
//! let raw = Photo::<Raw>::new(
//!     "IMG_001.JPG".to_string(),
//!     "/photos/IMG_001.JPG".to_string(),
//!     "2026-02-11 10:00:00".to_string(),
//! );
//! assert_eq!(raw.file_name(), "IMG_001.JPG");
//!
//! let classified = raw.classify(
//!     PhotoCategory::Construction,
//!     Some("舗装工".to_string()),
//!     "舗設状況".to_string(),
//! );
//! assert_eq!(classified.photo_category(), PhotoCategory::Construction);
//! ```
//!
//! ## 不正遷移はコンパイルエラー
//!
//! Raw 段階では photo_category にアクセスできない:
//! ```compile_fail
//! use photo_ai_common::domain::photo::{Photo, Raw};
//! let raw = Photo::<Raw>::new("a.jpg".to_string(), "".to_string(), "".to_string());
//! let _ = raw.photo_category(); // error: method not found
//! ```

use super::photo_category::PhotoCategory;
use super::station::Station;
use crate::types::{AnalysisResult, RawImageData};
use std::marker::PhantomData;

// ============================================================================
// Phase marker types
// ============================================================================

/// スキャン直後。画像ファイル情報と OCR 結果だけを持つ。
pub struct Raw;

/// AI 分類済。photo_category / work_type / description が確定。
pub struct Classified;

/// マスタ照合済。variety / subphase / remarks が確定。
pub struct Matched;

/// 正規化済。station（測点）・measurements が確定。
pub struct Normalized;

/// 出力可能。PDF/Excel/XML へレンダリング可能。
pub struct Exportable;

// ============================================================================
// Sealed trait: 外部から Phase を追加できないようにする
// ============================================================================

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::Raw {}
    impl Sealed for super::Classified {}
    impl Sealed for super::Matched {}
    impl Sealed for super::Normalized {}
    impl Sealed for super::Exportable {}
}

/// Phase 型であることを示す marker trait
pub trait PhotoPhase: sealed::Sealed {}

impl PhotoPhase for Raw {}
impl PhotoPhase for Classified {}
impl PhotoPhase for Matched {}
impl PhotoPhase for Normalized {}
impl PhotoPhase for Exportable {}

// ============================================================================
// Capability marker traits (階層構造)
// ============================================================================

/// Classified 以降で利用可能なアクセサを許可するマーカー
pub trait IsClassifiedOrLater: PhotoPhase {}
impl IsClassifiedOrLater for Classified {}
impl IsClassifiedOrLater for Matched {}
impl IsClassifiedOrLater for Normalized {}
impl IsClassifiedOrLater for Exportable {}

/// Matched 以降で利用可能なアクセサを許可するマーカー
pub trait IsMatchedOrLater: IsClassifiedOrLater {}
impl IsMatchedOrLater for Matched {}
impl IsMatchedOrLater for Normalized {}
impl IsMatchedOrLater for Exportable {}

/// Normalized 以降で利用可能なアクセサを許可するマーカー
pub trait IsNormalizedOrLater: IsMatchedOrLater {}
impl IsNormalizedOrLater for Normalized {}
impl IsNormalizedOrLater for Exportable {}

// ============================================================================
// Photo<P> 本体
// ============================================================================

/// 解析パイプラインの任意の段階にある写真
///
/// 内部は `AnalysisResult` を保持しており、`Into<AnalysisResult>` で既存 API と連結できる。
pub struct Photo<P: PhotoPhase> {
    inner: AnalysisResult,
    _phase: PhantomData<P>,
}

// --- すべての phase で利用可能なアクセサ ---

impl<P: PhotoPhase> Photo<P> {
    /// 現在 phase の内部 `AnalysisResult` を取り出す
    ///
    /// TypeState の線形遷移を飛ばすエスケープハッチ。既存 API（Vec<AnalysisResult>
    /// を返す関数）との連結用。理想的には `Photo<Exportable>::into_inner` の正規
    /// ルートで出口に到達すること。
    pub fn into_analysis_result(self) -> AnalysisResult {
        self.inner
    }

    pub fn file_name(&self) -> &str {
        &self.inner.file_name
    }

    pub fn file_path(&self) -> &str {
        &self.inner.file_path
    }

    pub fn date(&self) -> &str {
        &self.inner.date
    }

    pub fn has_board(&self) -> bool {
        self.inner.has_board
    }

    pub fn detected_text(&self) -> &str {
        &self.inner.detected_text
    }

    pub fn measurements(&self) -> &str {
        &self.inner.measurements
    }
}

// --- Classified 以降で利用可能 ---

impl<P: IsClassifiedOrLater> Photo<P> {
    /// 写真区分を enum で取得する
    ///
    /// Classified への遷移時に既知ラベルで初期化されるため、`expect` は安全。
    pub fn photo_category(&self) -> PhotoCategory {
        PhotoCategory::from_label(&self.inner.photo_category).unwrap_or_else(|| {
            panic!(
                "Classified phase invariant violated: unknown photo_category {:?}",
                self.inner.photo_category
            )
        })
    }

    pub fn work_type(&self) -> &str {
        &self.inner.work_type
    }

    pub fn description(&self) -> &str {
        &self.inner.description
    }
}

// --- Matched 以降で利用可能 ---

impl<P: IsMatchedOrLater> Photo<P> {
    pub fn variety(&self) -> &str {
        &self.inner.variety
    }

    pub fn subphase(&self) -> &str {
        &self.inner.subphase
    }

    pub fn remarks(&self) -> &str {
        &self.inner.remarks
    }
}

// --- Normalized 以降で利用可能 ---

impl<P: IsNormalizedOrLater> Photo<P> {
    /// station 文字列を Station enum に parse して返す
    pub fn station(&self) -> Station {
        Station::parse(&self.inner.station)
    }

    /// station を生の文字列として取得する（PDF/Excel 出力用の原文保持）
    pub fn station_str(&self) -> &str {
        &self.inner.station
    }
}

// ============================================================================
// Raw-only: 生成と classify 遷移
// ============================================================================

impl Photo<Raw> {
    /// 最小情報で Raw を組み立てる
    pub fn new(file_name: String, file_path: String, date: String) -> Self {
        Self {
            inner: AnalysisResult {
                file_name,
                file_path,
                date,
                ..Default::default()
            },
            _phase: PhantomData,
        }
    }

    /// `RawImageData` + EXIF 情報から Raw を組み立てる
    pub fn from_raw_data(raw: RawImageData, file_path: String, date: String) -> Self {
        Self {
            inner: AnalysisResult {
                file_name: raw.file_name,
                file_path,
                date,
                has_board: raw.has_board,
                detected_text: raw.detected_text,
                measurements: raw.measurements,
                description: raw.scene_description,
                photo_category: raw.photo_category, // Raw では参照できないが next 遷移で使う
                ..Default::default()
            },
            _phase: PhantomData,
        }
    }

    /// 分類を付与して Classified に遷移する
    ///
    /// `work_type` は動的集合（マスタ由来）なので文字列のまま受け取る。
    pub fn classify(
        mut self,
        category: PhotoCategory,
        work_type: Option<String>,
        description: String,
    ) -> Photo<Classified> {
        self.inner.photo_category = category.as_str().to_string();
        if let Some(wt) = work_type {
            self.inner.work_type = wt;
        }
        self.inner.description = description;
        Photo {
            inner: self.inner,
            _phase: PhantomData,
        }
    }
}

// ============================================================================
// Classified-only: with_master_match 遷移
// ============================================================================

impl Photo<Classified> {
    /// マスタ照合結果を反映して Matched に遷移する
    pub fn with_master_match(
        mut self,
        variety: String,
        subphase: String,
        remarks: String,
    ) -> Photo<Matched> {
        self.inner.variety = variety;
        self.inner.subphase = subphase;
        self.inner.remarks = remarks;
        Photo {
            inner: self.inner,
            _phase: PhantomData,
        }
    }

    /// マスタヒットなしでも Matched に進める fast path
    ///
    /// variety/subphase/remarks は空のまま Matched に遷移する。
    /// マスタなし工種（使用機械・安全管理写真の一部）で使用。
    pub fn into_matched_without_master(self) -> Photo<Matched> {
        Photo {
            inner: self.inner,
            _phase: PhantomData,
        }
    }
}

// ============================================================================
// Matched-only: normalize 遷移
// ============================================================================

impl Photo<Matched> {
    /// 測点と measurements を確定して Normalized に遷移する
    pub fn normalize(
        mut self,
        station: Station,
        measurements: Option<String>,
    ) -> Photo<Normalized> {
        self.inner.station = station.to_display();
        if let Some(m) = measurements {
            self.inner.measurements = m;
        }
        Photo {
            inner: self.inner,
            _phase: PhantomData,
        }
    }
}

// ============================================================================
// Normalized-only: finalize 遷移
// ============================================================================

impl Photo<Normalized> {
    /// 出力可能状態に最終化する
    ///
    /// 将来的にはここで出力前検証（必須フィールド・レイアウト整合性）を行う。
    /// 現状は型遷移のみ。
    pub fn finalize(self) -> Photo<Exportable> {
        Photo {
            inner: self.inner,
            _phase: PhantomData,
        }
    }
}

// ============================================================================
// Exportable-only: AnalysisResult との相互変換
// ============================================================================

impl Photo<Exportable> {
    /// 内部の `AnalysisResult` を取り出す（既存 API との連結用）
    pub fn into_inner(self) -> AnalysisResult {
        self.inner
    }

    /// 内部の `AnalysisResult` への参照を返す
    pub fn as_analysis_result(&self) -> &AnalysisResult {
        &self.inner
    }
}

// ============================================================================
// AnalysisResult との互換変換
// ============================================================================

impl From<AnalysisResult> for Photo<Raw> {
    /// 既存 `AnalysisResult` を「とりあえず Raw として受け入れる」
    ///
    /// 注意: これは互換のためのエスケープハッチで、型安全性を弱める。
    /// 既知の phase がわかるなら直接対応する From/コンストラクタを使うこと。
    fn from(inner: AnalysisResult) -> Self {
        Self {
            inner,
            _phase: PhantomData,
        }
    }
}

impl From<Photo<Exportable>> for AnalysisResult {
    fn from(photo: Photo<Exportable>) -> Self {
        photo.into_inner()
    }
}

// ============================================================================
// AnalysisResult → Photo<Matched>: バッチ分類結果の型付き昇格
// ============================================================================

impl TryFrom<AnalysisResult> for Photo<Matched> {
    /// 未知の `photo_category` が来た場合、元の `AnalysisResult` を保持して返す
    type Error = AnalysisResult;

    /// `analyze_batch_single_step` の結果を `Photo<Matched>` へ型付きで昇格する
    ///
    /// `photo_category` が `PhotoCategory::from_label` でマッチしなければ、元の
    /// `AnalysisResult` を `Err` で返す（ゴミを `Matched` に混ぜない）。
    /// マッチすれば variety/subphase/remarks が埋まっている前提で `Matched` phase に
    /// ラップする（`sanitize_classification` を通過した結果のみを想定する）。
    fn try_from(ar: AnalysisResult) -> Result<Self, Self::Error> {
        if PhotoCategory::from_label(&ar.photo_category).is_none() {
            return Err(ar);
        }
        Ok(Photo {
            inner: ar,
            _phase: PhantomData,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_raw() -> Photo<Raw> {
        Photo::<Raw>::new(
            "IMG_001.JPG".to_string(),
            "/tmp/IMG_001.JPG".to_string(),
            "2026-02-11 10:00:00".to_string(),
        )
    }

    fn sample_classified() -> Photo<Classified> {
        sample_raw().classify(
            PhotoCategory::Construction,
            Some("舗装工".to_string()),
            "舗設状況".to_string(),
        )
    }

    fn sample_matched() -> Photo<Matched> {
        sample_classified().with_master_match(
            "舗装打換え工".to_string(),
            "表層工".to_string(),
            "舗設状況".to_string(),
        )
    }

    fn sample_normalized() -> Photo<Normalized> {
        sample_matched().normalize(Station::parse("No.6 R"), None)
    }

    fn sample_exportable() -> Photo<Exportable> {
        sample_normalized().finalize()
    }

    // === 基本アクセサ ===

    #[test]
    fn raw_exposes_common_fields() {
        let p = sample_raw();
        assert_eq!(p.file_name(), "IMG_001.JPG");
        assert_eq!(p.file_path(), "/tmp/IMG_001.JPG");
        assert_eq!(p.date(), "2026-02-11 10:00:00");
        assert!(!p.has_board());
    }

    // === Phase 遷移 ===

    #[test]
    fn classify_moves_raw_to_classified() {
        let c = sample_classified();
        assert_eq!(c.photo_category(), PhotoCategory::Construction);
        assert_eq!(c.work_type(), "舗装工");
        assert_eq!(c.description(), "舗設状況");
    }

    #[test]
    fn with_master_match_moves_classified_to_matched() {
        let m = sample_matched();
        assert_eq!(m.variety(), "舗装打換え工");
        assert_eq!(m.subphase(), "表層工");
        assert_eq!(m.remarks(), "舗設状況");
        // Classified からのアクセサも保持されている
        assert_eq!(m.photo_category(), PhotoCategory::Construction);
    }

    #[test]
    fn into_matched_without_master_keeps_empty_fields() {
        let m = sample_classified().into_matched_without_master();
        assert_eq!(m.variety(), "");
        assert_eq!(m.subphase(), "");
        assert_eq!(m.remarks(), "");
    }

    #[test]
    fn normalize_moves_matched_to_normalized() {
        let n = sample_normalized();
        assert_eq!(n.station_str(), "No.6 R");
        assert!(n.station().is_post());
        // 上位 phase のアクセサも継承されている
        assert_eq!(n.photo_category(), PhotoCategory::Construction);
        assert_eq!(n.variety(), "舗装打換え工");
    }

    #[test]
    fn normalize_with_measurements_updates_field() {
        let n = sample_matched().normalize(
            Station::parse("2月11日 1台目"),
            Some("160.5℃".to_string()),
        );
        assert_eq!(n.measurements(), "160.5℃");
        assert!(n.station().is_date());
    }

    #[test]
    fn finalize_moves_normalized_to_exportable() {
        let e = sample_exportable();
        assert_eq!(e.station_str(), "No.6 R");
    }

    // === AnalysisResult との相互変換 ===

    #[test]
    fn exportable_into_analysis_result_preserves_all_fields() {
        let e = sample_exportable();
        let r: AnalysisResult = e.into();
        assert_eq!(r.file_name, "IMG_001.JPG");
        assert_eq!(r.photo_category, "施工状況写真");
        assert_eq!(r.work_type, "舗装工");
        assert_eq!(r.variety, "舗装打換え工");
        assert_eq!(r.subphase, "表層工");
        assert_eq!(r.station, "No.6 R");
    }

    #[test]
    fn analysis_result_into_raw_is_escape_hatch() {
        let r = AnalysisResult {
            file_name: "x.jpg".to_string(),
            photo_category: "安全管理写真".to_string(),
            ..Default::default()
        };
        // AnalysisResult → Photo<Raw> は互換のため許容。
        // Raw では photo_category() にアクセスできないので型安全は弱まるが、
        // 既存コードからの移行用エスケープハッチとして必要。
        let raw: Photo<Raw> = r.into();
        assert_eq!(raw.file_name(), "x.jpg");
    }

    #[test]
    fn from_raw_data_builds_raw_phase() {
        let r = RawImageData {
            file_name: "a.jpg".to_string(),
            has_board: true,
            detected_text: "工種：舗装工".to_string(),
            measurements: "160℃".to_string(),
            scene_description: "アスファルト舗装".to_string(),
            photo_category: "".to_string(),
        };
        let p = Photo::<Raw>::from_raw_data(r, "/tmp/a.jpg".into(), "2026-02-11".into());
        assert_eq!(p.file_name(), "a.jpg");
        assert!(p.has_board());
        assert_eq!(p.detected_text(), "工種：舗装工");
    }

    // === 線形遷移（full pipeline） ===

    #[test]
    fn full_pipeline_from_raw_to_exportable() {
        let e = Photo::<Raw>::new("a.jpg".into(), "/p/a.jpg".into(), "2026-02-11".into())
            .classify(PhotoCategory::Quality, Some("舗装工".into()), "温度管理".into())
            .with_master_match("舗装打換え工".into(), "表層工".into(), "到着温度測定".into())
            .normalize(Station::parse("2月11日 1台目"), Some("160℃".into()))
            .finalize();
        assert_eq!(e.photo_category(), PhotoCategory::Quality);
        assert_eq!(e.remarks(), "到着温度測定");
        assert!(e.station().is_date());
    }

    // === Station / PhotoCategory との連携 ===

    #[test]
    fn station_accessor_parses_enum() {
        let n = sample_matched().normalize(
            Station::parse("取付道路 No.3"),
            None,
        );
        match n.station() {
            Station::Post(p) => {
                assert!(p.access_road);
                assert_eq!(p.label, "No.3");
            }
            other => panic!("expected Post, got {:?}", other),
        }
    }

    #[test]
    fn photo_category_roundtrips_through_phases() {
        for cat in PhotoCategory::all() {
            let p = Photo::<Raw>::new("a.jpg".into(), "".into(), "".into()).classify(
                *cat,
                None,
                String::new(),
            );
            assert_eq!(p.photo_category(), *cat);
        }
    }

    // === Escape hatch ===

    #[test]
    fn into_analysis_result_works_at_any_phase() {
        // Raw phase でも escape hatch が効くこと
        let ar = sample_raw().into_analysis_result();
        assert_eq!(ar.file_name, "IMG_001.JPG");
        assert_eq!(ar.photo_category, ""); // Raw では未設定

        // Classified phase でも効くこと
        let ar = sample_classified().into_analysis_result();
        assert_eq!(ar.photo_category, "施工状況写真");
        assert_eq!(ar.variety, ""); // Classified では未設定
    }

    // === Classified fast path ===

    #[test]
    fn into_matched_without_master_allows_normalize() {
        // マスタヒットなしでも後続 phase に進めること
        let e = sample_classified()
            .into_matched_without_master()
            .normalize(Station::Empty, None)
            .finalize();
        assert_eq!(e.variety(), "");
        assert_eq!(e.remarks(), "");
        assert_eq!(e.station_str(), "");
    }

    // === TryFrom<AnalysisResult> for Photo<Matched> ===

    fn sample_matched_analysis_result() -> AnalysisResult {
        AnalysisResult {
            file_name: "IMG_123.JPG".to_string(),
            file_path: "/tmp/IMG_123.JPG".to_string(),
            date: "2026-02-11".to_string(),
            photo_category: "施工状況写真".to_string(),
            work_type: "舗装工".to_string(),
            variety: "舗装打換え工".to_string(),
            subphase: "表層工".to_string(),
            remarks: "舗設状況".to_string(),
            description: "アスファルト舗設".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn try_from_analysis_result_accepts_known_category() {
        let ar = sample_matched_analysis_result();
        let photo = match Photo::<Matched>::try_from(ar) {
            Ok(p) => p,
            Err(_) => panic!("known category should succeed"),
        };
        assert_eq!(photo.photo_category(), PhotoCategory::Construction);
        assert_eq!(photo.file_name(), "IMG_123.JPG");
    }

    #[test]
    fn try_from_analysis_result_rejects_unknown_category() {
        let ar = AnalysisResult {
            file_name: "x.jpg".to_string(),
            photo_category: "不明区分".to_string(),
            variety: "保持される".to_string(),
            ..Default::default()
        };
        // Err は元の AnalysisResult を保持していること
        match Photo::<Matched>::try_from(ar) {
            Ok(_) => panic!("unknown category should fail"),
            Err(original) => {
                assert_eq!(original.file_name, "x.jpg");
                assert_eq!(original.photo_category, "不明区分");
                assert_eq!(original.variety, "保持される");
            }
        }
    }

    #[test]
    fn try_from_analysis_result_rejects_empty_category() {
        let ar = AnalysisResult {
            file_name: "empty.jpg".to_string(),
            photo_category: String::new(),
            ..Default::default()
        };
        match Photo::<Matched>::try_from(ar) {
            Ok(_) => panic!("empty category should fail"),
            Err(original) => {
                assert_eq!(original.file_name, "empty.jpg");
                assert_eq!(original.photo_category, "");
            }
        }
    }

    #[test]
    fn try_from_analysis_result_preserves_matched_fields() {
        let ar = sample_matched_analysis_result();
        let photo = match Photo::<Matched>::try_from(ar) {
            Ok(p) => p,
            Err(_) => panic!("known category should succeed"),
        };
        // 共通アクセサ
        assert_eq!(photo.file_name(), "IMG_123.JPG");
        assert_eq!(photo.file_path(), "/tmp/IMG_123.JPG");
        assert_eq!(photo.date(), "2026-02-11");
        // Classified-level
        assert_eq!(photo.photo_category(), PhotoCategory::Construction);
        assert_eq!(photo.work_type(), "舗装工");
        assert_eq!(photo.description(), "アスファルト舗設");
        // Matched-level
        assert_eq!(photo.variety(), "舗装打換え工");
        assert_eq!(photo.subphase(), "表層工");
        assert_eq!(photo.remarks(), "舗設状況");
    }
}
