//! 工種キーワード定義データ
//!
//! DEFAULT_WORK_TYPE_DEFINITIONS を analyzer.rs のロジックから分離

use crate::analyzer::WorkTypeDefinition;

/// デフォルトの工種キーワード定義
pub const DEFAULT_WORK_TYPE_DEFINITIONS: &[WorkTypeDefinition] = &[
    WorkTypeDefinition {
        name: "舗装工",
        category_keywords: &["温度", "転圧", "舗設", "敷均し", "乳剤", "路盤"],
        text_keywords: &["アスファルト"],
        scene_keywords: &["アスファルト", "フィニッシャー", "ローラー"],
    },
    WorkTypeDefinition {
        name: "区画線工",
        category_keywords: &["区画線"],
        text_keywords: &["区画線", "ライン"],
        scene_keywords: &["白線", "区画線"],
    },
    WorkTypeDefinition {
        name: "構造物撤去工",
        category_keywords: &["取壊し"],
        text_keywords: &["撤去", "取壊"],
        scene_keywords: &["解体", "撤去"],
    },
    WorkTypeDefinition {
        name: "道路土工",
        category_keywords: &["掘削", "路床"],
        text_keywords: &["掘削"],
        scene_keywords: &["掘削", "バックホウ"],
    },
    WorkTypeDefinition {
        name: "排水構造物工",
        category_keywords: &[],
        text_keywords: &["側溝", "集水", "人孔"],
        scene_keywords: &["側溝", "マンホール"],
    },
    WorkTypeDefinition {
        name: "人孔改良工",
        category_keywords: &[],
        text_keywords: &["人孔改良", "マンホール蓋"],
        scene_keywords: &[],
    },
];
