//! エクスポートボタンコンポーネント

use leptos::prelude::*;
use crate::app::PhotoItem;

#[component]
pub fn ExportButtons<FA, FP, FE>(
    photos: ReadSignal<Vec<PhotoItem>>,
    is_analyzing: ReadSignal<bool>,
    on_analyze: FA,
    on_export_pdf: FP,
    on_export_excel: FE,
) -> impl IntoView
where
    FA: Fn(()) + 'static + Clone,
    FP: Fn(()) + 'static + Clone,
    FE: Fn(()) + 'static + Clone,
{
    let has_photos = move || !photos.get().is_empty();
    let all_analyzed = move || {
        let p = photos.get();
        !p.is_empty() && p.iter().all(|photo| {
            matches!(photo.status, crate::app::PhotoStatus::Done)
        })
    };

    view! {
        <div class="export-buttons">
            <button
                class="btn btn-primary"
                disabled=move || !has_photos() || is_analyzing.get()
                on:click={
                    let on_analyze = on_analyze.clone();
                    move |_| on_analyze(())
                }
            >
                {move || if is_analyzing.get() { "解析中..." } else { "AI解析開始" }}
            </button>

            <button
                class="btn btn-secondary"
                disabled=move || !all_analyzed()
                on:click={
                    let on_export_pdf = on_export_pdf.clone();
                    move |_| on_export_pdf(())
                }
            >
                "PDF出力"
            </button>

            <button
                class="btn btn-secondary"
                disabled=move || !all_analyzed()
                on:click={
                    let on_export_excel = on_export_excel.clone();
                    move |_| on_export_excel(())
                }
            >
                "Excel出力"
            </button>
        </div>
    }
}
