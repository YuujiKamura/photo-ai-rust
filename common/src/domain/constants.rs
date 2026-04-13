//! ドメイン定数
//!
//! 工事写真管理で使用する正規名称を一元管理する。
//! マジックストリングの重複・タイポを防ぐ。

// === 写真区分 ===

pub const PHOTO_CAT_SAFETY: &str = "安全管理写真";
pub const PHOTO_CAT_QUALITY: &str = "品質管理写真";
pub const PHOTO_CAT_CONSTRUCTION: &str = "施工状況写真";
pub const PHOTO_CAT_DEKIGATA: &str = "出来形管理写真";
pub const PHOTO_CAT_MATERIAL: &str = "使用材料写真";
pub const PHOTO_CAT_BEFORE_AFTER: &str = "着手前及び完成写真";
pub const PHOTO_CAT_OTHER: &str = "その他";

// === 工種 ===

pub const WORK_PAVEMENT: &str = "舗装工";
pub const WORK_LANE_MARKING: &str = "区画線工";

// === 種別 ===

pub const VARIETY_PAVEMENT_REPLACE: &str = "舗装打換え工";
pub const VARIETY_ROAD_CUTTING: &str = "路面切削工";
pub const VARIETY_CUTTING_OVERLAY: &str = "切削オーバーレイ工";

// === 細別 ===

pub const SUBPHASE_SURFACE: &str = "表層工";

// === 備考（業務ルールで頻出するもの） ===

/// 使用機械の備考
pub const REMARKS_MACHINERY: &str = "使用機械";
/// 重機始業前点検の備考
pub const REMARKS_MACHINERY_CHECK: &str = "重機始業前点検";

// === 安全管理系の備考（日付測点を自動設定する対象） ===

pub const REMARKS_SAFETY_MORNING: &str = "安全朝礼実施状況";
pub const REMARKS_SAFETY_KY: &str = "KY活動状況";
pub const REMARKS_SAFETY_NEW_ENTRY: &str = "新規入場者教育状況";
pub const REMARKS_SAFETY_TRAINING: &str = "安全訓練実施状況";
