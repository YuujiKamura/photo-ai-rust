//! プログレスバーコンポーネント

use leptos::prelude::*;

#[component]
pub fn ProgressBar(progress: ReadSignal<f32>) -> impl IntoView {
    view! {
        <div class="progress-container">
            <div class="progress-bar">
                <div
                    class="progress-fill"
                    style=move || format!("width: {}%", progress.get() * 100.0)
                />
            </div>
            <p class="progress-text">
                {move || format!("解析中... {:.0}%", progress.get() * 100.0)}
            </p>
        </div>
    }
}
