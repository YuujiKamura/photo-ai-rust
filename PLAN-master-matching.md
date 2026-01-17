# Phase 4 修正版: 既存マスタ構造との統合

## 既存コード確認結果

### データ構造（constructionHierarchyData.ts）

```
直接工事費
  └─ 写真区分（着手前及び完成写真、施工状況写真、品質管理写真...）
      └─ 工種（舗装工、区画線工...）
          └─ 種別（舗装打換え工、舗装版切断...）
              └─ 細別（表層工、上層路盤工...）
                  └─ 備考キー or { matchPatterns: [...] }
```

### matchPatterns の役割

- 末端ノードの特別なキー
- OCRテキストとマッチングするためのキーワード配列
- **マッチした場合、親キー（正式名称）を出力**

例:
```json
"品質管理写真": {
  "舗装工": {
    "舗装打換え工": {
      "表層工": {
        "アスファルト混合物温度測定": {
          "matchPatterns": ["温度管理", "合材温度", "到着温度"]
        }
      }
    }
  }
}
```

### 既存の照合ロジック（masterAdapter.ts）

1. `extractAllValidValues()` - 全階層から有効な値を抽出
2. `validateAgainstMaster()` - AI結果をマスタで検証
3. `detectUnknownTerms()` - 未知の用語（AI創作）を検出

---

## 私が間違えたこと

| 間違い | 正しい |
|--------|--------|
| Excelマスタ形式を創作 | 既存JSONマスタを使う |
| フラットな5列構造 | 5階層のネスト構造 |
| `variety`という列名 | 既存の変数名を踏襲 |

---

## 修正プラン

### Step 1: 共有JSONマスタの生成

TypeScriptで`constructionHierarchyData.ts`をJSON出力:

```bash
# GASPhotoAIManager側
npx ts-node -e "
  const { CONSTRUCTION_HIERARCHY } = require('./utils/constructionHierarchyData');
  console.log(JSON.stringify(CONSTRUCTION_HIERARCHY, null, 2));
" > shared/master.json
```

または、CLIのビルド時に自動生成。

### Step 2: Rustの型定義を修正

現在（間違い）:
```rust
pub struct MasterEntry {
    pub photo_category: String,
    pub work_type: String,
    pub variety: String,  // ← 勝手に命名
    pub detail: String,
    pub match_patterns: Vec<String>,
}
```

修正後:
```rust
// 既存構造に合わせたネスト型
// serde_json::Value で動的に処理
```

### Step 3: 照合ロジック

TypeScript版と同じ:
1. 階層を再帰的に走査
2. matchPatternsがあればキーワードマッチング
3. マッチしたら親キー（正式名称）を返す

### Step 4: calamine削除

Excelマスタは使わないので、calamineクレートは不要。

---

## ファイル変更

| ファイル | 変更内容 |
|----------|----------|
| `Cargo.toml` | calamine削除 |
| `src/matcher/mod.rs` | JSON読み込み、階層走査に変更 |
| `src/matcher/types.rs` | 削除（動的JSON処理に変更） |
| `cli.rs` | `-m`オプションの説明を修正 |

---

## 検証

```bash
# マスタJSONを生成
cd ../GASPhotoAIManager && npm run export-master

# Rustで照合テスト
cd ../photo-ai-rust
cargo run -- analyze ./test-photos -o result.json -m ../GASPhotoAIManager/shared/master.json
```
