//! 設定パネルコンポーネント

use leptos::prelude::*;

#[component]
pub fn SettingsPanel(
    api_key: ReadSignal<String>,
    set_api_key: WriteSignal<String>,
    title: ReadSignal<String>,
    set_title: WriteSignal<String>,
    photos_per_page: ReadSignal<u8>,
    set_photos_per_page: WriteSignal<u8>,
) -> impl IntoView {
    view! {
        <div class="settings-panel">
            <div class="settings-grid">
                <div class="form-group">
                    <label for="api-key">"Gemini API Key"</label>
                    <input
                        type="password"
                        id="api-key"
                        placeholder="API Keyを入力..."
                        prop:value=move || api_key.get()
                        on:input=move |ev| {
                            set_api_key.set(event_target_value(&ev));
                        }
                    />
                </div>

                <div class="form-group">
                    <label for="title">"台帳タイトル"</label>
                    <input
                        type="text"
                        id="title"
                        prop:value=move || title.get()
                        on:input=move |ev| {
                            set_title.set(event_target_value(&ev));
                        }
                    />
                </div>

                <div class="form-group">
                    <label for="photos-per-page">"写真枚数/ページ"</label>
                    <select
                        id="photos-per-page"
                        on:change=move |ev| {
                            let value: u8 = event_target_value(&ev).parse().unwrap_or(3);
                            set_photos_per_page.set(value);
                        }
                    >
                        <option value="2" selected=move || photos_per_page.get() == 2>"2枚"</option>
                        <option value="3" selected=move || photos_per_page.get() == 3>"3枚"</option>
                    </select>
                </div>
            </div>
        </div>
    }
}
