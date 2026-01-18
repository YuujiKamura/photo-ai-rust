//! メインアプリケーションコンポーネント

use leptos::prelude::*;
use crate::components::{
    header::Header,
    settings_panel::SettingsPanel,
    upload_area::UploadArea,
    photo_gallery::PhotoGallery,
    progress_bar::ProgressBar,
    export_buttons::ExportButtons,
};
use photo_ai_common::AnalysisResult;

/// アプリケーションの状態
#[derive(Clone, Default)]
pub struct AppState {
    pub api_key: String,
    pub title: String,
    pub photos_per_page: u8,
    pub photos: Vec<PhotoItem>,
    pub is_analyzing: bool,
    pub progress: f32,
}

/// 写真アイテム
#[derive(Clone)]
pub struct PhotoItem {
    pub id: String,
    pub file_name: String,
    pub data_url: String,
    pub status: PhotoStatus,
    pub analysis: Option<AnalysisResult>,
}

/// 写真ステータス
#[derive(Clone, Copy, PartialEq)]
pub enum PhotoStatus {
    Pending,
    Analyzing,
    Done,
    Error,
}

impl PhotoStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PhotoStatus::Pending => "pending",
            PhotoStatus::Analyzing => "analyzing",
            PhotoStatus::Done => "done",
            PhotoStatus::Error => "error",
        }
    }
}

/// メインアプリケーションコンポーネント
#[component]
pub fn App() -> impl IntoView {
    // アプリケーション状態
    let (api_key, set_api_key) = signal(String::new());
    let (title, set_title) = signal("工事写真台帳".to_string());
    let (photos_per_page, set_photos_per_page) = signal(3u8);
    let (photos, set_photos) = signal(Vec::<PhotoItem>::new());
    let (is_analyzing, set_is_analyzing) = signal(false);
    let (progress, set_progress) = signal(0.0f32);

    // 写真追加ハンドラ
    let on_photos_added = move |new_photos: Vec<PhotoItem>| {
        set_photos.update(|p| p.extend(new_photos));
    };

    // 解析開始ハンドラ
    let on_analyze = move |_| {
        // TODO: Gemini API呼び出し
        set_is_analyzing.set(true);
    };

    // PDF出力ハンドラ
    let on_export_pdf = move |_| {
        // TODO: PDF生成
    };

    // Excel出力ハンドラ
    let on_export_excel = move |_| {
        // TODO: Excel生成
    };

    view! {
        <div class="container">
            <Header />

            <SettingsPanel
                api_key=api_key
                set_api_key=set_api_key
                title=title
                set_title=set_title
                photos_per_page=photos_per_page
                set_photos_per_page=set_photos_per_page
            />

            <UploadArea on_photos_added=on_photos_added />

            <Show
                when=move || !photos.get().is_empty()
                fallback=|| view! { <p class="text-muted">"写真をドラッグ&ドロップまたはクリックしてアップロード"</p> }
            >
                <PhotoGallery photos=photos />
            </Show>

            <Show when=move || is_analyzing.get()>
                <ProgressBar progress=progress />
            </Show>

            <ExportButtons
                photos=photos
                is_analyzing=is_analyzing
                on_analyze=on_analyze
                on_export_pdf=on_export_pdf
                on_export_excel=on_export_excel
            />
        </div>
    }
}
