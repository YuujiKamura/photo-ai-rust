//! 写真区分
//!
//! 工事写真は7つの区分のいずれかに属する。閉じた集合なので enum で表現する。
//! 文字列ラベルとのラウンドトリップ変換は JSON/CSV/OCR 互換のために提供する。

use serde::{Deserialize, Serialize};

/// 写真区分
///
/// 国土交通省の工事写真管理要領に基づく7区分。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PhotoCategory {
    /// 安全管理写真
    Safety,
    /// 品質管理写真
    Quality,
    /// 施工状況写真
    Construction,
    /// 出来形管理写真
    Dekigata,
    /// 使用材料写真
    Material,
    /// 着手前及び完成写真
    BeforeAfter,
    /// その他（使用機械等）
    Other,
}

impl PhotoCategory {
    /// 日本語ラベル
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Safety => "安全管理写真",
            Self::Quality => "品質管理写真",
            Self::Construction => "施工状況写真",
            Self::Dekigata => "出来形管理写真",
            Self::Material => "使用材料写真",
            Self::BeforeAfter => "着手前及び完成写真",
            Self::Other => "その他",
        }
    }

    /// ラベル文字列から変換する
    ///
    /// 未知のラベル・空文字列は `None` を返す。
    pub fn from_label(s: &str) -> Option<Self> {
        match s {
            "安全管理写真" => Some(Self::Safety),
            "品質管理写真" => Some(Self::Quality),
            "施工状況写真" => Some(Self::Construction),
            "出来形管理写真" => Some(Self::Dekigata),
            "使用材料写真" => Some(Self::Material),
            "着手前及び完成写真" => Some(Self::BeforeAfter),
            "その他" => Some(Self::Other),
            _ => None,
        }
    }

    /// PDF/Excel 出力ファイル名用の略称
    ///
    /// `src/commands.rs::shorten_photo_category` 相当を enum 経由で提供する。
    pub const fn short_label(self) -> &'static str {
        match self {
            Self::Safety => "安全管理",
            Self::Quality => "品質管理",
            Self::Construction => "施工状況",
            Self::Dekigata => "出来形管理",
            Self::Material => "使用材料",
            Self::BeforeAfter => "着手前完成",
            Self::Other => "使用機械",
        }
    }

    /// 全区分を列挙する（UI 表示・ドロップダウン等に利用可能）
    pub const fn all() -> &'static [PhotoCategory] {
        &[
            Self::Safety,
            Self::Quality,
            Self::Construction,
            Self::Dekigata,
            Self::Material,
            Self::BeforeAfter,
            Self::Other,
        ]
    }
}

impl std::fmt::Display for PhotoCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for PhotoCategory {
    type Err = UnknownPhotoCategory;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_label(s).ok_or_else(|| UnknownPhotoCategory(s.to_string()))
    }
}

/// `FromStr` 失敗時のエラー
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownPhotoCategory(pub String);

impl std::fmt::Display for UnknownPhotoCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "未知の写真区分: {:?}", self.0)
    }
}

impl std::error::Error for UnknownPhotoCategory {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn all_7_categories_roundtrip_via_label() {
        for cat in PhotoCategory::all() {
            let label = cat.as_str();
            assert_eq!(PhotoCategory::from_label(label), Some(*cat));
        }
    }

    #[test]
    fn from_label_returns_none_for_unknown() {
        assert_eq!(PhotoCategory::from_label(""), None);
        assert_eq!(PhotoCategory::from_label("不明"), None);
        assert_eq!(PhotoCategory::from_label("安全管理"), None); // 略称は短縮形
    }

    #[test]
    fn from_str_returns_err_for_unknown() {
        let err = PhotoCategory::from_str("不明").unwrap_err();
        assert_eq!(err.0, "不明");
    }

    #[test]
    fn display_writes_full_label() {
        assert_eq!(format!("{}", PhotoCategory::Safety), "安全管理写真");
        assert_eq!(format!("{}", PhotoCategory::Other), "その他");
    }

    #[test]
    fn short_label_differs_from_full_label_except_other_semantics() {
        assert_eq!(PhotoCategory::Safety.short_label(), "安全管理");
        assert_eq!(PhotoCategory::Other.short_label(), "使用機械");
    }

    #[test]
    fn all_returns_exactly_seven_categories() {
        assert_eq!(PhotoCategory::all().len(), 7);
    }

    #[test]
    fn json_serializes_as_variant_name() {
        // serde_json のデフォルトで variant name が出る
        let json = serde_json::to_string(&PhotoCategory::Safety).unwrap();
        assert_eq!(json, "\"Safety\"");

        let restored: PhotoCategory = serde_json::from_str("\"Safety\"").unwrap();
        assert_eq!(restored, PhotoCategory::Safety);
    }

    #[test]
    fn all_labels_are_unique() {
        let labels: std::collections::HashSet<_> =
            PhotoCategory::all().iter().map(|c| c.as_str()).collect();
        assert_eq!(labels.len(), 7);
    }

    #[test]
    fn all_short_labels_are_unique() {
        let shorts: std::collections::HashSet<_> =
            PhotoCategory::all().iter().map(|c| c.short_label()).collect();
        assert_eq!(shorts.len(), 7);
    }
}
