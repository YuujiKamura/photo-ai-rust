//! 写真ギャラリーコンポーネント

use leptos::prelude::*;
use crate::app::PhotoItem;

#[component]
pub fn PhotoGallery(photos: ReadSignal<Vec<PhotoItem>>) -> impl IntoView {
    view! {
        <div class="photo-gallery">
            <For
                each=move || photos.get()
                key=|photo| photo.id.clone()
                children=move |photo| {
                    view! { <PhotoCard photo=photo /> }
                }
            />
        </div>
    }
}

#[component]
fn PhotoCard(photo: PhotoItem) -> impl IntoView {
    let status_class = photo.status.as_str();
    let status_text = match photo.status {
        crate::app::PhotoStatus::Pending => "待機中",
        crate::app::PhotoStatus::Analyzing => "解析中",
        crate::app::PhotoStatus::Done => "完了",
        crate::app::PhotoStatus::Error => "エラー",
    };

    view! {
        <div class="photo-card">
            <img src=photo.data_url.clone() alt=photo.file_name.clone() />
            <div class="photo-info">
                <h4>{photo.file_name.clone()}</h4>
                <span class=format!("photo-status {}", status_class)>
                    {status_text}
                </span>
                {photo.analysis.as_ref().map(|a| {
                    view! {
                        <p>{a.work_type.clone()}" / "{a.variety.clone()}</p>
                    }
                })}
            </div>
        </div>
    }
}
