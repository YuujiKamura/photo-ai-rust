//! エイリアス変換モジュール
//!
//! 写真区分や工種の表記ゆれを正規化する。

use crate::error::{Error, Result};
use crate::types::AnalysisResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 部分一致で最長マッチ変換を行う。
///
/// 完全一致を優先し、なければ最長の部分一致パターンで置換する。
/// どちらにも一致しなければ元の値をそのまま返す。
pub fn longest_match_transform(value: &str, map: &HashMap<String, String>) -> String {
    transform_field(value, map)
}

/// フィールド値をエイリアスマップで変換する。
///
/// 完全一致を優先し、なければ最長の部分一致パターンで置換する。
/// どちらにも一致しなければ元の値をそのまま返す。
fn transform_field(value: &str, map: &HashMap<String, String>) -> String {
    if value.is_empty() {
        return value.to_string();
    }

    // 完全一致を優先
    if let Some(replacement) = map.get(value) {
        return replacement.clone();
    }

    // 部分一致（最長マッチ）
    let mut best_match: Option<(&str, &str)> = None;
    for (pattern, replacement) in map {
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

/// エイリアス定義
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AliasConfig {
    #[serde(default)]
    photo_category: HashMap<String, String>,
    #[serde(default)]
    work_type: HashMap<String, String>,
    #[serde(default)]
    variety: HashMap<String, String>,
    #[serde(default)]
    subphase: HashMap<String, String>,
}

impl AliasConfig {
    /// 組み込みプリセットを取得（JSONから読み込み）
    ///
    /// 不明なプリセット名の場合は `Error::InvalidFormat` を返す。
    pub fn from_preset(name: &str) -> Result<Self> {
        let json = match name.to_lowercase().as_str() {
            "pavement" | "舗装" => include_str!("../../master/alias_presets/pavement.json"),
            "marking" | "区画線" => include_str!("../../master/alias_presets/marking.json"),
            "general" | "汎用" => include_str!("../../master/alias_presets/general.json"),
            _ => {
                return Err(Error::InvalidFormat {
                    key: "preset".to_string(),
                    detail: format!(
                        "不明なプリセット '{}' (pavement/marking/general)",
                        name
                    ),
                });
            }
        };
        let config: Self = serde_json::from_str(json)?;
        Ok(config)
    }

    /// プリセットとカスタムJSONからエイリアス設定を構築する。
    ///
    /// プリセットが不明な場合は警告を返し、エラーにはしない。
    /// カスタムJSONのパースに失敗した場合はエラーを返す。
    /// 返り値の第2要素は警告メッセージのリスト。
    pub fn build(preset: Option<&str>, alias_json: Option<&str>) -> Result<(Self, Vec<String>)> {
        let mut config = Self::default();
        let mut warnings = Vec::new();

        // プリセットを適用
        if let Some(preset_name) = preset {
            match Self::from_preset(preset_name) {
                Ok(preset_config) => config.merge(&preset_config),
                Err(Error::InvalidFormat { .. }) => {
                    warnings.push(format!(
                        "不明なプリセット '{}' (pavement/marking/general)",
                        preset_name
                    ));
                }
                Err(e) => return Err(e),
            }
        }

        // カスタムエイリアスJSONを適用（プリセットを上書き）
        if let Some(json) = alias_json {
            let custom_config = Self::from_json(json)?;
            config.merge(&custom_config);
        }

        Ok((config, warnings))
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

    /// 写真区分のエイリアスマップ
    pub fn photo_category(&self) -> &HashMap<String, String> {
        &self.photo_category
    }

    /// 工種のエイリアスマップ
    pub fn work_type(&self) -> &HashMap<String, String> {
        &self.work_type
    }

    /// 種別のエイリアスマップ
    pub fn variety(&self) -> &HashMap<String, String> {
        &self.variety
    }

    /// 作業段階のエイリアスマップ
    pub fn subphase(&self) -> &HashMap<String, String> {
        &self.subphase
    }

    /// 解析結果にエイリアス変換を適用
    pub fn apply(&self, result: &AnalysisResult) -> AnalysisResult {
        let mut updated = result.clone();

        updated.photo_category = transform_field(&result.photo_category, &self.photo_category);
        updated.work_type = transform_field(&result.work_type, &self.work_type);
        updated.variety = transform_field(&result.variety, &self.variety);
        updated.subphase = transform_field(&result.subphase, &self.subphase);

        updated
    }

    /// 設定をマージ（後から追加した設定が優先）
    pub fn merge(&mut self, other: &AliasConfig) {
        self.photo_category.extend(other.photo_category.clone());
        self.work_type.extend(other.work_type.clone());
        self.variety.extend(other.variety.clone());
        self.subphase.extend(other.subphase.clone());
    }
}

/// 解析結果にエイリアスを適用
///
/// 返り値の第2要素は警告メッセージのリスト。
pub fn apply_aliases(
    results: &[AnalysisResult],
    preset: Option<&str>,
    alias_json: Option<&str>,
) -> Result<(Vec<AnalysisResult>, Vec<String>)> {
    let (config, warnings) = AliasConfig::build(preset, alias_json)?;

    // 変換を適用
    let transformed: Vec<AnalysisResult> = results.iter().map(|r| config.apply(r)).collect();

    Ok((transformed, warnings))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pavement_preset() {
        let config = AliasConfig::from_preset("pavement").unwrap();
        assert_eq!(
            config.photo_category().get("品質"),
            Some(&"品質管理写真".to_string())
        );
        assert_eq!(
            config.work_type().get("舗装"),
            Some(&"舗装工".to_string())
        );
    }

    #[test]
    fn test_unknown_preset_returns_error() {
        let result = AliasConfig::from_preset("unknown");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, Error::InvalidFormat { .. }));
        assert!(format!("{}", err).contains("unknown"));
    }

    #[test]
    fn test_transform_exact_match() {
        let config = AliasConfig::from_preset("pavement").unwrap();

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
        let config = AliasConfig::from_preset("pavement").unwrap();

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

        let (transformed, warnings) =
            apply_aliases(&results, Some("pavement"), None).unwrap();

        assert_eq!(transformed[0].photo_category, "品質管理写真");
        assert_eq!(transformed[1].photo_category, "出来形管理写真");
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_apply_aliases_unknown_preset_warning() {
        let results = vec![AnalysisResult {
            file_name: "test.jpg".to_string(),
            photo_category: "品質".to_string(),
            ..Default::default()
        }];

        let (transformed, warnings) =
            apply_aliases(&results, Some("unknown"), None).unwrap();

        // No transformation happened (unknown preset)
        assert_eq!(transformed[0].photo_category, "品質");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("unknown"));
    }

    #[test]
    fn test_longest_match_transform_standalone() {
        let mut map = HashMap::new();
        map.insert("品質".to_string(), "品質管理写真".to_string());
        map.insert("品質管理".to_string(), "品質管理写真".to_string());

        // Exact match preferred
        assert_eq!(longest_match_transform("品質", &map), "品質管理写真");
        // Longest partial match
        assert_eq!(longest_match_transform("品質管理", &map), "品質管理写真");
        // No match returns original
        assert_eq!(longest_match_transform("その他", &map), "その他");
        // Empty returns empty
        assert_eq!(longest_match_transform("", &map), "");
    }

    #[test]
    fn test_preset_from_json_file() {
        let json = r#"{"photo_category": {"テスト": "テスト写真"}, "work_type": {}, "variety": {}, "subphase": {}}"#;
        let config = AliasConfig::from_json(json).unwrap();
        assert_eq!(
            config.photo_category().get("テスト"),
            Some(&"テスト写真".to_string())
        );
    }

    #[test]
    fn test_accessor_methods() {
        let config = AliasConfig::from_preset("marking").unwrap();
        assert!(config.photo_category().contains_key("品質"));
        assert!(config.work_type().contains_key("区画線"));
        assert!(config.variety().contains_key("溶融式"));
        assert!(config.subphase().is_empty());
    }

    #[test]
    fn test_merge() {
        let mut base = AliasConfig::from_preset("general").unwrap();
        let pavement = AliasConfig::from_preset("pavement").unwrap();
        base.merge(&pavement);
        // Pavement-specific entries are now in the merged config
        assert!(base.work_type().contains_key("舗装"));
        // General entries are preserved
        assert!(base.photo_category().contains_key("着工"));
    }

    #[test]
    fn test_build_with_preset() {
        let (config, warnings) = AliasConfig::build(Some("pavement"), None).unwrap();
        assert!(warnings.is_empty());
        assert_eq!(
            config.photo_category().get("品質"),
            Some(&"品質管理写真".to_string())
        );
    }

    #[test]
    fn test_build_with_unknown_preset_warns() {
        let (config, warnings) = AliasConfig::build(Some("unknown"), None).unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("unknown"));
        // Config is empty (no preset applied)
        assert!(config.photo_category().is_empty());
    }

    #[test]
    fn test_build_with_custom_json() {
        let json = r#"{"photo_category": {"テスト": "テスト写真"}}"#;
        let (config, warnings) = AliasConfig::build(None, Some(json)).unwrap();
        assert!(warnings.is_empty());
        assert_eq!(
            config.photo_category().get("テスト"),
            Some(&"テスト写真".to_string())
        );
    }

    #[test]
    fn test_build_preset_and_custom_merge() {
        let json = r#"{"photo_category": {"カスタム": "カスタム写真"}}"#;
        let (config, warnings) = AliasConfig::build(Some("pavement"), Some(json)).unwrap();
        assert!(warnings.is_empty());
        // Preset entries present
        assert_eq!(
            config.photo_category().get("品質"),
            Some(&"品質管理写真".to_string())
        );
        // Custom entries merged
        assert_eq!(
            config.photo_category().get("カスタム"),
            Some(&"カスタム写真".to_string())
        );
    }
}
