//! メインアプリケーションコンポーネント

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::console;
use crate::components::{
    header::Header,
    settings_panel::SettingsPanel,
    upload_area::UploadArea,
    photo_gallery::PhotoGallery,
    progress_bar::ProgressBar,
    export_buttons::ExportButtons,
};
use crate::export::{excel_wasm, pdf_wasm};
use crate::export::js_bindings::{download_excel_js, download_pdf_js};
use crate::secure_store::{clear_api_key, decrypt_api_key, encrypt_api_key};
use photo_ai_common::AnalysisResult;
use std::collections::HashSet;
use crate::api::gemini::analyze_batch;

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
    pub pair_id: Option<String>,
    pub pair_order: Option<u8>,
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

fn build_export_results(photos: &[PhotoItem]) -> Vec<AnalysisResult> {
    photos
        .iter()
        .filter_map(|photo| {
            photo.analysis.clone().map(|mut analysis| {
                analysis.file_name = photo.file_name.clone();
                analysis.file_path = photo.data_url.clone();
                analysis
            })
        })
        .collect()
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
    let (pairing_first, set_pairing_first) = signal(None::<String>);
    let (selected_ids, set_selected_ids) = signal(HashSet::<String>::new());
    let (station_value, set_station_value) = signal(String::new());
    let (station_prefix, set_station_prefix) = signal(String::new());
    let (station_start, set_station_start) = signal(1u32);
    let (station_step, set_station_step) = signal(1u32);
    let (logs, set_logs) = signal(Vec::<String>::new());
    let (passphrase, set_passphrase) = signal(String::new());
    let (api_key_status, set_api_key_status) = signal(String::new());

    let push_log = {
        let set_logs = set_logs.clone();
        move |message: String| {
            set_logs.update(|items| {
                items.push(message);
            });
        }
    };

    // 写真追加ハンドラ
    let on_photos_added = move |new_photos: Vec<PhotoItem>| {
        let added_count = new_photos.len();
        set_photos.update(|p| p.extend(new_photos));
        push_log(format!("写真を追加: {}枚", added_count));
    };

    // 解析開始ハンドラ
    let on_analyze = {
        let api_key = api_key.clone();
        let photos = photos.clone();
        let set_photos = set_photos.clone();
        let set_is_analyzing = set_is_analyzing.clone();
        let set_progress = set_progress.clone();
        let push_log = push_log.clone();
        move |_| {
            let key = api_key.get();
            if key.trim().is_empty() {
                push_log("APIキーが未設定のため解析を開始できません".to_string());
                return;
            }

            let photo_list = photos.get();
            if photo_list.is_empty() {
                push_log("写真がありません".to_string());
                return;
            }

            let total = photo_list.len();
            push_log(format!("AI解析を開始 ({}枚)", total));
            set_is_analyzing.set(true);
            set_progress.set(0.0);

            let batch_input: Vec<(String, String, String)> = photo_list
                .iter()
                .map(|p| (p.id.clone(), p.data_url.clone(), p.file_name.clone()))
                .collect();

            spawn_local(async move {
                let key = key.clone();
                let results = analyze_batch(&key, batch_input, move |current, total| {
                    let pct = if total == 0 { 0.0 } else { (current as f32 / total as f32) * 100.0 };
                    set_progress.set(pct);
                    push_log(format!("解析中: {}/{} ({}%)", current, total, pct.round()));
                }).await;

                set_photos.update(|photos| {
                    for (id, result) in results {
                        if let Some(photo) = photos.iter_mut().find(|p| p.id == id) {
                            match result {
                                Ok(analysis) => {
                                    photo.status = crate::app::PhotoStatus::Done;
                                    photo.analysis = Some(analysis);
                                }
                                Err(err) => {
                                    photo.status = crate::app::PhotoStatus::Error;
                                    push_log(format!("解析失敗: {} ({})", photo.file_name, err));
                                }
                            }
                        }
                    }
                });

                set_is_analyzing.set(false);
                push_log("AI解析が完了しました".to_string());
            });
        }
    };

    let on_save_api_key = {
        let api_key = api_key.clone();
        let passphrase = passphrase.clone();
        let set_api_key_status = set_api_key_status.clone();
        let push_log = push_log.clone();
        move |_| {
            let key = api_key.get();
            let phrase = passphrase.get();
            if key.trim().is_empty() || phrase.trim().is_empty() {
                set_api_key_status.set("APIキーとパスフレーズを入力してください".to_string());
                return;
            }

            spawn_local(async move {
                match encrypt_api_key(&key, &phrase).await {
                    Ok(()) => {
                        set_api_key_status.set("APIキーを暗号化保存しました".to_string());
                        push_log("APIキーを暗号化保存".to_string());
                    }
                    Err(err) => {
                        set_api_key_status.set(err);
                    }
                }
            });
        }
    };

    let on_load_api_key = {
        let passphrase = passphrase.clone();
        let set_api_key = set_api_key.clone();
        let set_api_key_status = set_api_key_status.clone();
        let push_log = push_log.clone();
        move |_| {
            let phrase = passphrase.get();
            if phrase.trim().is_empty() {
                set_api_key_status.set("パスフレーズを入力してください".to_string());
                return;
            }

            spawn_local(async move {
                match decrypt_api_key(&phrase).await {
                    Ok(key) => {
                        set_api_key.set(key);
                        set_api_key_status.set("APIキーを復号しました".to_string());
                        push_log("APIキーを復号".to_string());
                    }
                    Err(err) => {
                        set_api_key_status.set(err);
                    }
                }
            });
        }
    };

    let on_clear_api_key = {
        let set_api_key_status = set_api_key_status.clone();
        let push_log = push_log.clone();
        move |_| {
            clear_api_key();
            set_api_key_status.set("保存済みAPIキーを削除しました".to_string());
            push_log("保存済みAPIキーを削除".to_string());
        }
    };

    let on_pair_select = {
        let set_photos = set_photos.clone();
        let pairing_first = pairing_first.clone();
        let set_pairing_first = set_pairing_first.clone();
        move |photo_id: String| {
            if let Some(first_id) = pairing_first.get() {
                if first_id == photo_id {
                    set_pairing_first.set(None);
                    push_log("ペア選択をキャンセル".to_string());
                    return;
                }

                let pair_id = format!("pair-{}", js_sys::Date::now());
                set_photos.update(|photos| {
                    let mut cleared_pairs = std::collections::HashSet::new();
                    for photo in photos.iter_mut() {
                        if photo.id == first_id || photo.id == photo_id {
                            if let Some(existing) = photo.pair_id.take() {
                                cleared_pairs.insert(existing);
                            }
                            photo.pair_order = None;
                        }
                    }

                    if !cleared_pairs.is_empty() {
                        for photo in photos.iter_mut() {
                            if let Some(existing) = photo.pair_id.clone() {
                                if cleared_pairs.contains(&existing) {
                                    photo.pair_id = None;
                                    photo.pair_order = None;
                                }
                            }
                        }
                    }

                    let mut first_idx = None;
                    let mut second_idx = None;
                    for (idx, photo) in photos.iter().enumerate() {
                        if photo.id == first_id {
                            first_idx = Some(idx);
                        }
                        if photo.id == photo_id {
                            second_idx = Some(idx);
                        }
                    }

                    if let (Some(first), Some(second)) = (first_idx, second_idx) {
                        if first != second {
                            let second_photo = photos.remove(second);
                            let insert_pos = if second > first { first + 1 } else { first };
                            photos.insert(insert_pos, second_photo);
                        }
                    }

                    for photo in photos.iter_mut() {
                        if photo.id == first_id {
                            photo.pair_id = Some(pair_id.clone());
                            photo.pair_order = Some(1);
                        }
                        if photo.id == photo_id {
                            photo.pair_id = Some(pair_id.clone());
                            photo.pair_order = Some(2);
                        }
                    }
                });

                set_pairing_first.set(None);
                push_log("写真をペア化".to_string());
            } else {
                set_pairing_first.set(Some(photo_id));
                push_log("ペアの1枚目を選択".to_string());
            }
        }
    };

    let on_pair_clear = {
        let set_photos = set_photos.clone();
        move |photo_id: String| {
            set_photos.update(|photos| {
                let mut target_pair = None;
                for photo in photos.iter_mut() {
                    if photo.id == photo_id {
                        target_pair = photo.pair_id.take();
                        photo.pair_order = None;
                        break;
                    }
                }

                if let Some(pair_id) = target_pair {
                    for photo in photos.iter_mut() {
                        if photo.pair_id.as_deref() == Some(pair_id.as_str()) {
                            photo.pair_id = None;
                            photo.pair_order = None;
                        }
                    }
                }
            });
            push_log("ペアを解除".to_string());
        }
    };

    let on_reorder = {
        let set_photos = set_photos.clone();
        move |from_id: String, to_id: String| {
            if from_id == to_id {
                return;
            }

            set_photos.update(|photos| {
                let mut from_idx = None;
                let mut to_idx = None;

                for (idx, photo) in photos.iter().enumerate() {
                    if photo.id == from_id {
                        from_idx = Some(idx);
                    }
                    if photo.id == to_id {
                        to_idx = Some(idx);
                    }
                }

                let (Some(from), Some(to)) = (from_idx, to_idx) else {
                    return;
                };

                let photo = photos.remove(from);
                let insert_pos = if from < to { to - 1 } else { to };
                photos.insert(insert_pos, photo);
            });
            push_log("写真の並び替え".to_string());
        }
    };

    let on_toggle_select = {
        let set_selected_ids = set_selected_ids.clone();
        move |photo_id: String| {
            set_selected_ids.update(|ids| {
                if ids.contains(&photo_id) {
                    ids.remove(&photo_id);
                } else {
                    ids.insert(photo_id);
                }
            });
            push_log("写真の選択を更新".to_string());
        }
    };

    let on_select_all = {
        let photos = photos.clone();
        let set_selected_ids = set_selected_ids.clone();
        move |_| {
            let ids: HashSet<String> = photos.get().iter().map(|p| p.id.clone()).collect();
            set_selected_ids.set(ids);
            push_log("全選択".to_string());
        }
    };

    let on_clear_selection = {
        let set_selected_ids = set_selected_ids.clone();
        move |_| {
            set_selected_ids.set(HashSet::new());
            push_log("選択解除".to_string());
        }
    };

    let on_apply_station_fixed = {
        let station_value = station_value.clone();
        let selected_ids = selected_ids.clone();
        let set_photos = set_photos.clone();
        move |_| {
            let station = station_value.get();
            if station.trim().is_empty() {
                push_log("固定測点が空のため中止".to_string());
                return;
            }
            let selected = selected_ids.get();
            set_photos.update(|photos| {
                for photo in photos.iter_mut() {
                    if selected.contains(&photo.id) {
                        let analysis = photo.analysis.get_or_insert_with(|| AnalysisResult {
                            file_name: photo.file_name.clone(),
                            ..Default::default()
                        });
                        analysis.station = station.clone();
                    }
                }
            });
            push_log(format!("固定測点を適用: {}", station));
        }
    };

    let on_apply_station_sequence = {
        let station_prefix = station_prefix.clone();
        let station_start = station_start.clone();
        let station_step = station_step.clone();
        let selected_ids = selected_ids.clone();
        let set_photos = set_photos.clone();
        move |_| {
            let prefix = station_prefix.get();
            let start = station_start.get();
            let step = station_step.get().max(1);
            let selected = selected_ids.get();
            if selected.is_empty() {
                push_log("連番の適用対象がありません".to_string());
                return;
            }

            set_photos.update(|photos| {
                let mut counter = start;
                for photo in photos.iter_mut() {
                    if selected.contains(&photo.id) {
                        let analysis = photo.analysis.get_or_insert_with(|| AnalysisResult {
                            file_name: photo.file_name.clone(),
                            ..Default::default()
                        });
                        analysis.station = format!("{}{}", prefix, counter);
                        counter = counter.saturating_add(step);
                    }
                }
            });
            push_log("連番測点を適用".to_string());
        }
    };

    // PDF出力ハンドラ
    let on_export_pdf = move |_| {
        let photo_items = photos.get();
        let title_value = title.get();
        let per_page = photos_per_page.get();

        let results = build_export_results(&photo_items);
        if results.is_empty() {
            console::warn_1(&"No analyzed photos available for PDF export.".into());
            return;
        }

        let export_title = if title_value.trim().is_empty() {
            "工事写真台帳".to_string()
        } else {
            title_value
        };

        spawn_local(async move {
            match pdf_wasm::generate_pdf(&results, &export_title, per_page).await {
                Ok(data) => {
                    let filename = format!("{}.pdf", export_title);
                    download_pdf_js(&data, &filename);
                }
                Err(err) => {
                    console::error_1(&format!("PDF export failed: {}", err).into());
                }
            }
        });
    };

    // Excel出力ハンドラ
    let on_export_excel = move |_| {
        let photo_items = photos.get();
        let title_value = title.get();
        let per_page = photos_per_page.get();

        let results = build_export_results(&photo_items);
        if results.is_empty() {
            console::warn_1(&"No analyzed photos available for Excel export.".into());
            return;
        }

        let export_title = if title_value.trim().is_empty() {
            "工事写真台帳".to_string()
        } else {
            title_value
        };

        spawn_local(async move {
            match excel_wasm::generate_excel(&results, &export_title, per_page).await {
                Ok(data) => {
                    let filename = format!("{}.xlsx", export_title);
                    download_excel_js(&data, &filename);
                }
                Err(err) => {
                    console::error_1(&format!("Excel export failed: {}", err).into());
                }
            }
        });
    };

    view! {
        <div class="container">
            <Header />

            <SettingsPanel
                api_key=api_key
                set_api_key=set_api_key
                passphrase=passphrase
                set_passphrase=set_passphrase
                api_key_status=api_key_status
                on_save_api_key=on_save_api_key
                on_load_api_key=on_load_api_key
                on_clear_api_key=on_clear_api_key
                title=title
                set_title=set_title
                photos_per_page=photos_per_page
                set_photos_per_page=set_photos_per_page
            />

            <UploadArea api_key=api_key on_photos_added=on_photos_added />

            <Show
                when=move || !photos.get().is_empty()
                fallback=|| view! { <p class="text-muted">"写真をドラッグ&ドロップまたはクリックしてアップロード"</p> }
            >
                <div class="bulk-panel">
                    <div class="bulk-header">
                        <div class="bulk-title">"測点の一括適用"</div>
                        <div class="bulk-actions">
                            <span class="bulk-count">
                                {move || format!("選択中: {}枚", selected_ids.get().len())}
                            </span>
                            <button class="btn btn-tertiary btn-small" on:click=on_select_all>
                                "全選択"
                            </button>
                            <button class="btn btn-tertiary btn-small" on:click=on_clear_selection>
                                "選択解除"
                            </button>
                        </div>
                    </div>

                    <div class="bulk-grid">
                        <div class="bulk-card">
                            <div class="bulk-card-title">"固定測点"</div>
                            <input
                                class="bulk-input"
                                type="text"
                                placeholder="例: No.10"
                                prop:value=move || station_value.get()
                                on:input=move |ev| set_station_value.set(event_target_value(&ev))
                            />
                            <button
                                class="btn btn-primary btn-small"
                                disabled=move || selected_ids.get().is_empty()
                                on:click=on_apply_station_fixed
                            >
                                "選択に適用"
                            </button>
                        </div>

                        <div class="bulk-card">
                            <div class="bulk-card-title">"連番測点"</div>
                            <div class="bulk-row">
                                <input
                                    class="bulk-input"
                                    type="text"
                                    placeholder="プレフィックス (例: No.)"
                                    prop:value=move || station_prefix.get()
                                    on:input=move |ev| set_station_prefix.set(event_target_value(&ev))
                                />
                                <input
                                    class="bulk-input bulk-number"
                                    type="number"
                                    min="0"
                                    prop:value=move || station_start.get().to_string()
                                    on:input=move |ev| {
                                        let value = event_target_value(&ev).parse().unwrap_or(1);
                                        set_station_start.set(value);
                                    }
                                />
                                <input
                                    class="bulk-input bulk-number"
                                    type="number"
                                    min="1"
                                    prop:value=move || station_step.get().to_string()
                                    on:input=move |ev| {
                                        let value = event_target_value(&ev).parse().unwrap_or(1);
                                        set_station_step.set(value);
                                    }
                                />
                            </div>
                            <div class="bulk-hint">"プレフィックス + 連番を順に適用"</div>
                            <button
                                class="btn btn-primary btn-small"
                                disabled=move || selected_ids.get().is_empty()
                                on:click=on_apply_station_sequence
                            >
                                "連番を適用"
                            </button>
                        </div>
                    </div>
                </div>
                <PhotoGallery
                    photos=photos
                    pairing_first=pairing_first
                    on_pair_select=on_pair_select
                    on_pair_clear=on_pair_clear
                    on_reorder=on_reorder
                    selected_ids=selected_ids
                    on_toggle_select=on_toggle_select
                />
            </Show>

            <div class="log-console">
                <div class="log-header">
                    <span>"詳細ログ"</span>
                    <button
                        class="btn btn-tertiary btn-small"
                        on:click=move |_| set_logs.set(Vec::new())
                    >
                        "クリア"
                    </button>
                </div>
                <div class="log-body">
                    <For
                        each=move || logs.get()
                        key=|entry| entry.clone()
                        children=move |entry| {
                            view! { <div class="log-line">{entry}</div> }
                        }
                    />
                </div>
            </div>

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
