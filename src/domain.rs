//! ドメイン定数モジュール
//!
//! 工事写真管理で使用する定数を一元管理する。
//! 複数モジュールで参照されるドメイン文字列をここに集約し、
//! タイポや不整合を防ぐ。

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
