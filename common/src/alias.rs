//! エイリアス変換モジュール
//!
//! 写真区分や工種の表記ゆれを正規化する。

use crate::types::AnalysisResult;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// エイリアス定義
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AliasConfig {
    /// 写真区分のエイリアス
    #[serde(default)]
    pub photo_category: HashMap<String, String>,
    /// 工種のエイリアス
    #[serde(default)]
    pub work_type: HashMap<String, String>,
    /// 種別のエイリアス
    #[serde(default)]
    pub variety: HashMap<String, String>,
    /// 細別のエイリアス
    #[serde(default)]
    pub detail: HashMap<String, String>,
}

impl AliasConfig {
    /// 組み込みプリセットを取得
    pub fn from_preset(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "pavement" | "舗装" => Some(Self::pavement_preset()),
            "marking" | "区画線" => Some(Self::marking_preset()),
            "general" | "汎用" => Some(Self::general_preset()),
            _ => None,
        }
    }

    /// JSONファイルから読み込み（非WASM環境のみ）
    #[cfg(not(feature = "wasm"))]
    pub fn from_file(path: &std::path::Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// JSON文字列から読み込み
    pub fn from_json(json: &str) -> Result<Self> {
        let config: Self = serde_json::from_str(json)?;
        Ok(config)
    }

    /// 舗装工事用プリセット
    fn pavement_preset() -> Self {
        let mut config = Self::default();

        // 写真区分
        config.photo_category.insert("品質".into(), "品質管理写真".into());
        config.photo_category.insert("品質管理".into(), "品質管理写真".into());
        config.photo_category.insert("出来形".into(), "出来形管理写真".into());
        config.photo_category.insert("出来形管理".into(), "出来形管理写真".into());
        config.photo_category.insert("施工状況".into(), "施工状況写真".into());
        config.photo_category.insert("施工中".into(), "施工状況写真".into());
        config.photo_category.insert("安全".into(), "安全管理写真".into());
        config.photo_category.insert("安全管理".into(), "安全管理写真".into());
        config.photo_category.insert("材料".into(), "使用材料写真".into());
        config.photo_category.insert("使用材料".into(), "使用材料写真".into());

        // 工種
        config.work_type.insert("舗装".into(), "舗装工".into());
        config.work_type.insert("As".into(), "舗装工".into());
        config.work_type.insert("アスファルト".into(), "舗装工".into());

        // 種別
        config.variety.insert("打換え".into(), "舗装打換え工".into());
        config.variety.insert("打換".into(), "舗装打換え工".into());
        config.variety.insert("オーバーレイ".into(), "舗装オーバーレイ工".into());

        // 細別
        config.detail.insert("表層".into(), "表層工".into());
        config.detail.insert("基層".into(), "基層工".into());
        config.detail.insert("上層路盤".into(), "上層路盤工".into());
        config.detail.insert("下層路盤".into(), "下層路盤工".into());

        config
    }

    /// 区画線工事用プリセット
    fn marking_preset() -> Self {
        let mut config = Self::default();

        // 写真区分
        config.photo_category.insert("品質".into(), "品質管理写真".into());
        config.photo_category.insert("出来形".into(), "出来形管理写真".into());
        config.photo_category.insert("施工状況".into(), "施工状況写真".into());

        // 工種
        config.work_type.insert("区画線".into(), "区画線工".into());
        config.work_type.insert("ライン".into(), "区画線工".into());
        config.work_type.insert("白線".into(), "区画線工".into());

        // 種別
        config.variety.insert("溶融式".into(), "溶融式区画線".into());
        config.variety.insert("ペイント".into(), "ペイント式区画線".into());

        config
    }

    /// 汎用プリセット
    fn general_preset() -> Self {
        let mut config = Self::default();

        // 写真区分のみ
        config.photo_category.insert("品質".into(), "品質管理写真".into());
        config.photo_category.insert("出来形".into(), "出来形管理写真".into());
        config.photo_category.insert("施工".into(), "施工状況写真".into());
        config.photo_category.insert("安全".into(), "安全管理写真".into());
        config.photo_category.insert("材料".into(), "使用材料写真".into());
        config.photo_category.insert("着工".into(), "着工前写真".into());
        config.photo_category.insert("完成".into(), "完成写真".into());

        config
    }

    /// フィールドを変換（部分一致で最長マッチ）
    fn transform_field(&self, value: &str, aliases: &HashMap<String, String>) -> String {
        if value.is_empty() {
            return value.to_string();
        }

        // 完全一致を優先
        if let Some(replacement) = aliases.get(value) {
            return replacement.clone();
        }

        // 部分一致（最長マッチ）
        let mut best_match: Option<(&str, &str)> = None;
        for (pattern, replacement) in aliases {
            if value.contains(pattern.as_str())
                && (best_match.is_none() || pattern.len() > best_match.unwrap().0.len())
            {
                best_match = Some((pattern.as_str(), replacement.as_str()));
            }
        }

        if let Some((_, replacement)) = best_match {
            replacement.to_string()
        } else {
            value.to_string()
        }
    }

    /// 解析結果にエイリアス変換を適用
    pub fn apply(&self, result: &AnalysisResult) -> AnalysisResult {
        let mut updated = result.clone();

        updated.photo_category = self.transform_field(&result.photo_category, &self.photo_category);
        updated.work_type = self.transform_field(&result.work_type, &self.work_type);
        updated.variety = self.transform_field(&result.variety, &self.variety);
        updated.detail = self.transform_field(&result.detail, &self.detail);

        updated
    }

    /// 設定をマージ（後から追加した設定が優先）
    pub fn merge(&mut self, other: &AliasConfig) {
        self.photo_category.extend(other.photo_category.clone());
        self.work_type.extend(other.work_type.clone());
        self.variety.extend(other.variety.clone());
        self.detail.extend(other.detail.clone());
    }
}

/// 解析結果にエイリアスを適用
pub fn apply_aliases(
    results: &[AnalysisResult],
    preset: Option<&str>,
    alias_json: Option<&str>,
) -> Result<Vec<AnalysisResult>> {
    // エイリアス設定を構築
    let mut config = AliasConfig::default();

    // プリセットを適用
    if let Some(preset_name) = preset {
        if let Some(preset_config) = AliasConfig::from_preset(preset_name) {
            config.merge(&preset_config);
        } else {
            eprintln!("警告: 不明なプリセット '{}' (pavement/marking/general)", preset_name);
        }
    }

    // カスタムエイリアスJSONを適用（プリセットを上書き）
    if let Some(json) = alias_json {
        let custom_config = AliasConfig::from_json(json)?;
        config.merge(&custom_config);
    }

    // 変換を適用
    let transformed: Vec<AnalysisResult> = results
        .iter()
        .map(|r| config.apply(r))
        .collect();

    Ok(transformed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pavement_preset() {
        let config = AliasConfig::from_preset("pavement").unwrap();
        assert_eq!(config.photo_category.get("品質"), Some(&"品質管理写真".to_string()));
        assert_eq!(config.work_type.get("舗装"), Some(&"舗装工".to_string()));
    }

    #[test]
    fn test_transform_exact_match() {
        let config = AliasConfig::pavement_preset();

        let result = AnalysisResult {
            file_name: "test.jpg".to_string(),
            photo_category: "品質".to_string(),
            work_type: "舗装".to_string(),
            ..Default::default()
        };

        let transformed = config.apply(&result);
        assert_eq!(transformed.photo_category, "品質管理写真");
        assert_eq!(transformed.work_type, "舗装工");
    }

    #[test]
    fn test_transform_partial_match() {
        let config = AliasConfig::pavement_preset();

        let result = AnalysisResult {
            file_name: "test.jpg".to_string(),
            photo_category: "品質管理".to_string(),
            ..Default::default()
        };

        let transformed = config.apply(&result);
        assert_eq!(transformed.photo_category, "品質管理写真");
    }

    #[test]
    fn test_apply_aliases() {
        let results = vec![
            AnalysisResult {
                file_name: "test1.jpg".to_string(),
                photo_category: "品質".to_string(),
                ..Default::default()
            },
            AnalysisResult {
                file_name: "test2.jpg".to_string(),
                photo_category: "出来形".to_string(),
                ..Default::default()
            },
        ];

        let transformed = apply_aliases(&results, Some("pavement"), None).unwrap();

        assert_eq!(transformed[0].photo_category, "品質管理写真");
        assert_eq!(transformed[1].photo_category, "出来形管理写真");
    }
}
