//! 写真ギャラリーコンポーネント

use leptos::prelude::*;
use crate::app::PhotoItem;
use std::collections::HashSet;

#[component]
pub fn PhotoGallery<FP, FC, FR, FS>(
    photos: ReadSignal<Vec<PhotoItem>>,
    pairing_first: ReadSignal<Option<String>>,
    on_pair_select: FP,
    on_pair_clear: FC,
    on_reorder: FR,
    selected_ids: ReadSignal<HashSet<String>>,
    on_toggle_select: FS,
) -> impl IntoView
where
    FP: Fn(String) + 'static + Clone + Send,
    FC: Fn(String) + 'static + Clone + Send,
    FR: Fn(String, String) + 'static + Clone + Send,
    FS: Fn(String) + 'static + Clone + Send,
{
    let (dragging_id, set_dragging_id) = signal(None::<String>);
    let (drag_over_id, set_drag_over_id) = signal(None::<String>);

    view! {
        <div class="photo-gallery">
            <For
                each=move || photos.get()
                key=|photo| photo.id.clone()
                children=move |photo| {
                    let on_pair_select = on_pair_select.clone();
                    let on_pair_clear = on_pair_clear.clone();
                    let pairing_first = pairing_first.clone();
                    let on_reorder = on_reorder.clone();
                    let dragging_id = dragging_id.clone();
                    let set_dragging_id = set_dragging_id.clone();
                    let drag_over_id = drag_over_id.clone();
                    let set_drag_over_id = set_drag_over_id.clone();
                    let selected_ids = selected_ids.clone();
                    let on_toggle_select = on_toggle_select.clone();
                    view! {
                        <PhotoCard
                            photo=photo
                            pairing_first=pairing_first
                            on_pair_select=on_pair_select
                            on_pair_clear=on_pair_clear
                            on_reorder=on_reorder
                            dragging_id=dragging_id
                            set_dragging_id=set_dragging_id
                            drag_over_id=drag_over_id
                            set_drag_over_id=set_drag_over_id
                            selected_ids=selected_ids
                            on_toggle_select=on_toggle_select
                        />
                    }
                }
            />
        </div>
    }
}

#[component]
fn PhotoCard<FP, FC, FR, FS>(
    photo: PhotoItem,
    pairing_first: ReadSignal<Option<String>>,
    on_pair_select: FP,
    on_pair_clear: FC,
    on_reorder: FR,
    dragging_id: ReadSignal<Option<String>>,
    set_dragging_id: WriteSignal<Option<String>>,
    drag_over_id: ReadSignal<Option<String>>,
    set_drag_over_id: WriteSignal<Option<String>>,
    selected_ids: ReadSignal<HashSet<String>>,
    on_toggle_select: FS,
) -> impl IntoView
where
    FP: Fn(String) + 'static + Clone + Send,
    FC: Fn(String) + 'static + Clone + Send,
    FR: Fn(String, String) + 'static + Clone + Send,
    FS: Fn(String) + 'static + Clone + Send,
{
    let status_class = photo.status.as_str();
    let status_text = match photo.status {
        crate::app::PhotoStatus::Pending => "待機中",
        crate::app::PhotoStatus::Analyzing => "解析中",
        crate::app::PhotoStatus::Done => "完了",
        crate::app::PhotoStatus::Error => "エラー",
    };

    let is_pairing_first = {
        let photo_id = photo.id.clone();
        move || pairing_first.get().as_deref() == Some(photo_id.as_str())
    };

    let is_dragging = {
        let photo_id = photo.id.clone();
        move || dragging_id.get().as_deref() == Some(photo_id.as_str())
    };

    let is_drag_over = {
        let photo_id = photo.id.clone();
        move || drag_over_id.get().as_deref() == Some(photo_id.as_str())
    };

    let is_selected = {
        let photo_id = photo.id.clone();
        move || selected_ids.get().contains(&photo_id)
    };

    let pair_label = match photo.pair_order {
        Some(1) => "ペア 1/2",
        Some(2) => "ペア 2/2",
        _ => "未ペア",
    };
    let has_pair = photo.pair_id.is_some();

    let is_pairing_first_class = is_pairing_first.clone();
    let is_pairing_first_label = is_pairing_first.clone();
    let is_selected_class = is_selected.clone();
    let is_selected_checkbox = is_selected.clone();

    view! {
        <div
            class="photo-card"
            class:pairing=is_pairing_first_class
            class:dragging=is_dragging
            class:drag-over=is_drag_over
            class:selected=is_selected_class
            draggable="true"
            on:dragstart={
                let photo_id = photo.id.clone();
                let set_dragging_id = set_dragging_id.clone();
                move |_| {
                    set_dragging_id.set(Some(photo_id.clone()));
                }
            }
            on:dragend={
                let set_dragging_id = set_dragging_id.clone();
                let set_drag_over_id = set_drag_over_id.clone();
                move |_| {
                    set_dragging_id.set(None);
                    set_drag_over_id.set(None);
                }
            }
            on:dragover={
                let photo_id = photo.id.clone();
                let set_drag_over_id = set_drag_over_id.clone();
                move |ev| {
                    ev.prevent_default();
                    set_drag_over_id.set(Some(photo_id.clone()));
                }
            }
            on:dragleave={
                let set_drag_over_id = set_drag_over_id.clone();
                move |_| {
                    set_drag_over_id.set(None);
                }
            }
            on:drop={
                let photo_id = photo.id.clone();
                let dragging_id = dragging_id.clone();
                let set_dragging_id = set_dragging_id.clone();
                let set_drag_over_id = set_drag_over_id.clone();
                let on_reorder = on_reorder.clone();
                move |ev| {
                    ev.prevent_default();
                    if let Some(from_id) = dragging_id.get() {
                        if from_id != photo_id {
                            on_reorder(from_id, photo_id.clone());
                        }
                    }
                    set_dragging_id.set(None);
                    set_drag_over_id.set(None);
                }
            }
        >
            <img src=photo.data_url.clone() alt=photo.file_name.clone() />
            <div class="photo-info">
                <h4>{photo.file_name.clone()}</h4>
                <div class="photo-meta">
                    <span class=format!("photo-status {}", status_class)>
                        {status_text}
                    </span>
                    <span class="pair-badge">{pair_label}</span>
                </div>
                {photo.analysis.as_ref().map(|a| {
                    view! {
                        <p>{a.work_type.clone()}" / "{a.variety.clone()}</p>
                        <p>{format!("測点: {}", if a.station.is_empty() { "-" } else { &a.station })}</p>
                    }
                })}
                <div class="photo-actions">
                    <label class="select-pill">
                        <input
                            type="checkbox"
                            checked={
                                let is_selected = is_selected_checkbox.clone();
                                move || is_selected()
                            }
                            on:change={
                                let on_toggle_select = on_toggle_select.clone();
                                let photo_id = photo.id.clone();
                                move |_| on_toggle_select(photo_id.clone())
                            }
                        />
                        "選択"
                    </label>
                    <button
                        class="btn btn-small btn-secondary"
                        on:click={
                            let on_pair_select = on_pair_select.clone();
                            let photo_id = photo.id.clone();
                            move |_| on_pair_select(photo_id.clone())
                        }
                    >
                        {move || {
                            let is_pairing_first = is_pairing_first_label.clone();
                            if is_pairing_first() {
                                "1枚目選択済み"
                            } else if has_pair {
                                "ペア再選択"
                            } else {
                                "ペアを選ぶ"
                            }
                        }}
                    </button>
                    <button
                        class="btn btn-small btn-tertiary"
                        disabled=!has_pair
                        on:click={
                            let on_pair_clear = on_pair_clear.clone();
                            let photo_id = photo.id.clone();
                            move |_| on_pair_clear(photo_id.clone())
                        }
                    >
                        "ペア解除"
                    </button>
                </div>
            </div>
        </div>
    }
}
