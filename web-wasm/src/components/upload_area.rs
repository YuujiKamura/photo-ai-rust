//! アップロードエリアコンポーネント

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use web_sys::{DragEvent, File, FileList, FileReader};
use crate::app::{PhotoItem, PhotoStatus};

#[component]
pub fn UploadArea<F>(
    api_key: ReadSignal<String>,
    on_photos_added: F,
) -> impl IntoView
where
    F: Fn(Vec<PhotoItem>) + 'static + Clone,
{
    let (is_dragover, set_is_dragover) = signal(false);
    let is_enabled = move || !api_key.get().is_empty();

    let handle_files = {
        let on_photos_added = on_photos_added.clone();
        move |files: FileList| {
            let on_photos_added = on_photos_added.clone();
            for i in 0..files.length() {
                if let Some(file) = files.get(i) {
                    read_file(file, on_photos_added.clone());
                }
            }
        }
    };

    let on_drop = {
        let handle_files = handle_files.clone();
        move |ev: DragEvent| {
            ev.prevent_default();
            set_is_dragover.set(false);

            if !is_enabled() {
                return;
            }

            if let Some(dt) = ev.data_transfer() {
                if let Some(files) = dt.files() {
                    handle_files(files);
                }
            }
        }
    };

    let on_dragover = move |ev: DragEvent| {
        ev.prevent_default();
        if is_enabled() {
            set_is_dragover.set(true);
        }
    };

    let on_dragleave = move |_: DragEvent| {
        set_is_dragover.set(false);
    };

    let on_click = {
        let handle_files = handle_files.clone();
        move |_| {
            if !is_enabled() {
                return;
            }

            // ファイル選択ダイアログを開く
            let document = web_sys::window().unwrap().document().unwrap();
            let input: web_sys::HtmlInputElement = document
                .create_element("input")
                .unwrap()
                .dyn_into()
                .unwrap();
            input.set_type("file");
            input.set_accept("image/*");
            input.set_multiple(true);

            let handle_files = handle_files.clone();
            let closure = Closure::wrap(Box::new(move |_: web_sys::Event| {
                let input: web_sys::HtmlInputElement = web_sys::window()
                    .unwrap()
                    .document()
                    .unwrap()
                    .query_selector("input[type=file]")
                    .unwrap()
                    .unwrap()
                    .dyn_into()
                    .unwrap();
                if let Some(files) = input.files() {
                    handle_files(files);
                }
            }) as Box<dyn FnMut(_)>);

            input.set_onchange(Some(closure.as_ref().unchecked_ref()));
            closure.forget();
            input.click();
        }
    };

    view! {
        <div
            class=move || {
                let mut classes = vec!["upload-area"];
                if is_dragover.get() {
                    classes.push("dragover");
                }
                if !is_enabled() {
                    classes.push("disabled");
                }
                classes.join(" ")
            }
            on:drop=on_drop
            on:dragover=on_dragover
            on:dragleave=on_dragleave
            on:click=on_click
        >
            <Show
                when=is_enabled
                fallback=|| view! {
                    <div class="upload-icon">"🔑"</div>
                    <p>"APIキーを入力してください"</p>
                    <p class="text-muted">"上の設定欄でGemini APIキーを設定すると写真をアップロードできます"</p>
                }
            >
                <div class="upload-icon">"📷"</div>
                <p>"写真をドラッグ&ドロップ または クリックして選択"</p>
                <p class="text-muted">"対応形式: JPEG, PNG"</p>
            </Show>
        </div>
    }
}

fn read_file<F>(file: File, on_photo_added: F)
where
    F: Fn(Vec<PhotoItem>) + 'static,
{
    let file_name = file.name();
    let reader = FileReader::new().unwrap();

    let file_name_clone = file_name.clone();
    let reader_clone = reader.clone();
    let closure = Closure::wrap(Box::new(move |_: web_sys::ProgressEvent| {
        if let Ok(result) = reader_clone.result() {
            if let Some(data_url) = result.as_string() {
                let photo = PhotoItem {
                    id: format!("{}-{}", file_name_clone, js_sys::Date::now()),
                    file_name: file_name_clone.clone(),
                    data_url,
                    status: PhotoStatus::Pending,
                    analysis: None,
                };
                on_photo_added(vec![photo]);
            }
        }
    }) as Box<dyn FnMut(_)>);

    reader.set_onload(Some(closure.as_ref().unchecked_ref()));
    closure.forget();

    let _ = reader.read_as_data_url(&file);
}
