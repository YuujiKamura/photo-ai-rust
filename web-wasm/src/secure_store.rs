//! Encrypted API key storage via WebCrypto

use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/js/secure-store.js")]
extern "C" {
    #[wasm_bindgen(js_name = "encryptApiKey", catch)]
    async fn encrypt_api_key_js(api_key: &str, passphrase: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = "decryptApiKey", catch)]
    async fn decrypt_api_key_js(passphrase: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = "clearApiKey")]
    fn clear_api_key_js();
}

pub async fn encrypt_api_key(api_key: &str, passphrase: &str) -> Result<(), String> {
    encrypt_api_key_js(api_key, passphrase)
        .await
        .map(|_| ())
        .map_err(|e| format!("保存失敗: {:?}", e))
}

pub async fn decrypt_api_key(passphrase: &str) -> Result<String, String> {
    let value = decrypt_api_key_js(passphrase)
        .await
        .map_err(|e| format!("読込失敗: {:?}", e))?;
    value
        .as_string()
        .ok_or_else(|| "読込失敗: 文字列に変換できません".to_string())
}

pub fn clear_api_key() {
    clear_api_key_js();
}
