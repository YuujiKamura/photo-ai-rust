//! ヘッダーコンポーネント

use leptos::prelude::*;

#[component]
pub fn Header() -> impl IntoView {
    view! {
        <header class="header">
            <h1>"Photo AI - 工事写真台帳生成"</h1>
        </header>
    }
}
