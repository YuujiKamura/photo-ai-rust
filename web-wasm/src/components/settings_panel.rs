//! 設定パネルコンポーネント

use leptos::prelude::*;

#[component]
pub fn SettingsPanel<FS, FL, FC>(
    api_key: ReadSignal<String>,
    set_api_key: WriteSignal<String>,
    passphrase: ReadSignal<String>,
    set_passphrase: WriteSignal<String>,
    api_key_status: ReadSignal<String>,
    on_save_api_key: FS,
    on_load_api_key: FL,
    on_clear_api_key: FC,
    title: ReadSignal<String>,
    set_title: WriteSignal<String>,
    photos_per_page: ReadSignal<u8>,
    set_photos_per_page: WriteSignal<u8>,
) -> impl IntoView
where
    FS: Fn(()) + 'static + Clone,
    FL: Fn(()) + 'static + Clone,
    FC: Fn(()) + 'static + Clone,
{
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
                    <a
                        href="https://aistudio.google.com/app/apikey"
                        target="_blank"
                        rel="noopener noreferrer"
                        class="api-key-link"
                    >
                        "APIキーを取得 →"
                    </a>
                </div>

                <div class="form-group">
                    <label for="passphrase">"APIキー暗号化パスフレーズ"</label>
                    <input
                        type="password"
                        id="passphrase"
                        placeholder="パスフレーズを入力..."
                        prop:value=move || passphrase.get()
                        on:input=move |ev| {
                            set_passphrase.set(event_target_value(&ev));
                        }
                    />
                    <div class="api-actions">
                        <button
                            class="btn btn-secondary btn-small"
                            on:click={
                                let on_load_api_key = on_load_api_key.clone();
                                move |_| on_load_api_key(())
                            }
                        >
                            "読込"
                        </button>
                        <button
                            class="btn btn-primary btn-small"
                            on:click={
                                let on_save_api_key = on_save_api_key.clone();
                                move |_| on_save_api_key(())
                            }
                        >
                            "保存"
                        </button>
                        <button
                            class="btn btn-tertiary btn-small"
                            on:click={
                                let on_clear_api_key = on_clear_api_key.clone();
                                move |_| on_clear_api_key(())
                            }
                        >
                            "削除"
                        </button>
                    </div>
                    <div class="api-key-status">
                        {move || api_key_status.get()}
                    </div>
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
