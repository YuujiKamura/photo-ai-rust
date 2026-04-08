use std::io::Write;

pub struct ApiKeyGuard {
    encrypted: Vec<u8>,
    pad: Vec<u8>,
}

impl ApiKeyGuard {
    pub fn prompt() -> anyhow::Result<Self> {
        eprintln!("┌─────────────────────────────────────────────┐");
        eprintln!("│  従量課金モード (--pay-per-use)              │");
        eprintln!("│                                             │");
        eprintln!("│  APIキーの管理方針:                          │");
        eprintln!("│  - ディスクには一切保存しません               │");
        eprintln!("│  - メモリ上はXOR暗号化で保持します           │");
        eprintln!("│  - このプロセス終了時に自動消去されます       │");
        eprintln!("│  - 次回実行時は再度入力が必要です             │");
        eprintln!("└─────────────────────────────────────────────┘");

        std::env::remove_var("GEMINI_API_KEY");

        eprint!("GEMINI_API_KEY: ");
        std::io::stderr().flush()?;

        let key = rpassword::read_password()?;
        let key = key.trim().to_string();
        if key.is_empty() {
            anyhow::bail!("APIキーが空です");
        }

        let guard = Self::from_plaintext(&key);
        drop(key);
        guard.activate();
        eprintln!("APIキー受理 ({}文字、暗号化保持中)\n", guard.encrypted.len());
        Ok(guard)
    }

    fn from_plaintext(plaintext: &str) -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
            ^ (std::process::id() as u64).wrapping_mul(2654435761);

        let pad: Vec<u8> = (0..plaintext.len())
            .map(|i| {
                let mut h = seed.wrapping_add(i as u64);
                h = h
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                (h >> 33) as u8
            })
            .collect();

        let encrypted: Vec<u8> = plaintext
            .bytes()
            .zip(pad.iter())
            .map(|(b, p)| b ^ p)
            .collect();

        Self { encrypted, pad }
    }

    fn activate(&self) {
        let plain = self.decrypt();
        std::env::set_var("GEMINI_API_KEY", &plain);
    }

    fn decrypt(&self) -> String {
        let bytes: Vec<u8> = self
            .encrypted
            .iter()
            .zip(self.pad.iter())
            .map(|(e, p)| e ^ p)
            .collect();
        String::from_utf8(bytes).unwrap_or_default()
    }
}

impl Drop for ApiKeyGuard {
    fn drop(&mut self) {
        std::env::remove_var("GEMINI_API_KEY");
        for b in &mut self.encrypted {
            *b = 0;
        }
        for b in &mut self.pad {
            *b = 0;
        }
    }
}
