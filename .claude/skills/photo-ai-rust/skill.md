---
description: "Rust製工事写真AI解析CLIツール(v0.3.0)。(1) Gemini/Claude/Codexで写真解析・黒板OCR、(2) photo-taggerによるグループ化、(3) 工種階層マスタとの照合、(4) 測定値の正規化、(5) PDF/Excel/XML出力。写真台帳の一括生成パイプライン。"
keywords:
  - photo-ai
  - photo-ai-rust
  - 工事写真
  - 写真解析
  - 写真台帳
  - photo-tagger
  - OCR
  - 黒板
  - 温度管理
  - 品質管理
  - Excel出力
  - PDF出力
  - Rust
  - Gemini
  - 工種マスタ
---

# photo-ai-rust スキル (v0.3.0)

## 概要
Rust製の工事写真AI解析CLIツール。写真をスキャン→photo-taggerでグループ化→AI解析→工種マスタ照合→正規化→PDF/Excel出力。

## リポジトリ・パス
- GitHub: https://github.com/YuujiKamura/photo-ai-rust
- ローカル: `C:\Users\yuuji\photo-ai-rust`
- バイナリ: `photo-ai-rust/target/release/photo-ai`

## ワークスペース構成
```
photo-ai-rust/
├── src/           # メインCLIバイナリ
├── common/        # 共有ライブラリ (photo-ai-common)
└── web-wasm/      # Web/WASM版（予定）
```

## CLIコマンド一覧

### グローバルフラグ
```
-v, --verbose              詳細ログ
--ai-provider <PROVIDER>   claude / codex / gemini (デフォルト: gemini)
```

### run — フルパイプライン（スキャン→解析→出力）
```bash
photo-ai run <FOLDER> [OPTIONS]
  -o, --output <PATH>        出力先
  --format <FORMAT>          pdf / excel / xml / both (デフォルト: pdf)
  -b, --batch-size <N>       バッチサイズ (デフォルト: 5)
  -m, --master <CSV>         工種マスタCSV
  -w, --work-type <STRING>   工種指定（1段階解析）
  -t, --photo-type <STRING>  写真区分指定
  --variety <STRING>         種別指定
  -s, --station <STRING>     測点を全写真に適用
  --pdf-quality <QUALITY>    high / medium / low (デフォルト: medium)
  --use-cache                キャッシュ利用
  -r, --recursive            サブフォルダも走査
  --include-all              除外フォルダも含める
```

### analyze — 解析のみ（JSON出力）
```bash
photo-ai analyze <FOLDER> -o result.json [OPTIONS]
```

### export — JSON → PDF/Excel変換
```bash
photo-ai export result.json --format both [OPTIONS]
  --preset <NAME>            エイリアスプリセット: pavement / marking / general
  --alias <JSON>             カスタムエイリアスファイル
  -p, --photos-per-page <N>  2 or 3 (デフォルト: 3)
```

### normalize — 測定値の正規化
```bash
photo-ai normalize result.json [OPTIONS]
  --dry-run                  プレビューのみ
  -S, --station <STRING>     測点を全写真に適用
```

### station — 測点の対話入力
```bash
photo-ai station result.json
```

### cache — キャッシュ管理
```bash
photo-ai cache --info          # 情報表示
photo-ai cache --clear         # 削除
```

### review — ソースコードレビュー
```bash
photo-ai review <PATH> [-w]   # -w: 変更監視
```

## 典型的な使い方

### 一発で写真台帳を作る
```bash
cd C:/Users/yuuji/photo-ai-rust
cargo run --release -- run "H:\マイドライブ\〇市道 南千反畑町第１号線舗装補修工事\１５工事写真\Photomanager\20260211" --format both -m master/construction_hierarchy.csv
```

### 2段階ワークフロー
```bash
# Step 1: 解析
photo-ai analyze ./photos -o result.json --use-cache

# Step 2: 出力（何度でもやり直せる）
photo-ai export result.json --format pdf --preset pavement -o 舗装工台帳.pdf
```

### 工種指定で高速解析
```bash
photo-ai run ./photos -w 舗装工 -t 品質管理写真 --format both
```

## 解析パイプライン
```
1. scan     → フォルダ走査 + EXIF日付抽出
2. tagger   → photo-tagger でグループ化（機材・工程別）+ 黒板OCR
3. master   → detected_text(黒板OCR)からキー:値抽出 → 工種マスタ照合
4. normalize→ グループ内の測定値統一
5. export   → PDF/Excel/XML 生成
```

### マスタ照合ロジック（analysis.rs → master_matcher.rs）

#### Step 0: 安全管理写真の直接判定（safety_remarks_from_machine_type）
photo-taggerのmachine_type（グループ名）から安全管理系を判定:
- 朝礼 / 安全ミーティング → photoCategory="安全管理写真", remarks="安全朝礼実施状況"
- KY → "KY活動状況"
- 新規入場者教育 → "新規入場者教育状況"
- 黒板なし写真でもtagger情報で正しく分類できる
- **station自動設定**: 安全管理写真でstationが空の場合、撮影日から「X月Y日」を自動設定

#### Step 1: detected_text正規化（ocr_parser.rs）
photo-taggerが返すdetected_textは**3つの異なるフォーマット**で返ることがある:
- `\n`区切り（正常系）
- `, `カンマ区切り
- スペース区切り（「場所：No.6 R 表層工 初期転圧状況」等）
- リテラル`\n`（バックスラッシュ+n の2文字）

正規化パイプライン:
1. `normalize_detected_text`: リテラル`\n`→実改行変換、既知キー(工事名/場所/工種等)+コロン前に改行挿入
2. `extract_kv_from_text`: 改行分割→カンマ展開→キー:値抽出、場所値からstation部分とキーワード部分を分離

#### Step 2: マスタ照合（master_matcher.rs: match_master_from_detected_texts）
黒板OCRの`detected_text` + taggerの`role`からマスタ照合:
1. Step 1で抽出した`キー:値`ペアからキーワードを収集
2. `工種:舗装工` → work_typeでマスタ行をフィルタ
3. **variety_hint**: 黒板テキストから抽出された種別/細別名でマスタ候補をハードフィルタ
   - ソフトスコアリング（ボーナス加点）方式は別工種の誤マッチを招くため却下済み
4. **Phase 1**: マスタの検索パターン列 → キーワードでregexマッチ（優先）
5. **Phase 2**: マスタの備考列 → bigramトークン重複スコアで照合（フォールバック）
   - **focus_target boost** (Issue #95): taggerのrole（写真の視覚的内容）を`focus_target`として渡し、`token_overlap_score(focus_target, remarks) * 3`のブーストを加算
6. マッチ失敗時 → 工種だけ埋め、フォルダ名を備考に使用

#### focus_target boostの仕組み（Issue #95）
黒板OCRだけでは語順違い・省略で誤マッチする場合がある:
- 黒板「切削・積込状況」→ bigram照合で「切削殻積込状況」(score 4) > 「路面切削状況」(score 2)
- しかし写真の見た目（taggerのrole）は「路面切削状況」

focus_target boostにより、taggerの視覚判定をPhase 2スコアに反映:
```rust
let ft_boost = token_overlap_score(focus_target, &row.remarks) * 3;
let score = kw_score + ft_boost;
```

**制約**: taggerのroleがマスタ備考と語彙的に近い必要がある。自由記述だとbigramが効かない。
taggerが誤判定した場合は逆効果になる。倍率3は経験的な値。

### アーキテクチャ課題: 自由解析 vs 被写体リスト選択型（Issue #94）

#### 現状の問題（ゆらぎの根本原因）
```
写真 → AI自由解析(detected_text) → テキストパース → キーワード照合 → マスタ
       ↑フォーマットが不安定        ↑パッチ増殖
```
1. **AI出力フォーマットの不安定**: Geminiが3+形式で返す → パーサーの分岐が増える
2. **暗黙ルールの後付けパッチ増殖**: 安全ミーティング→朝礼、station→日付 等
3. **variety_hintのジレンマ**: ハードフィルタ→想定外が通らない、ソフト→誤マッチ

#### 重要な事実
AIの視覚認識はほぼ正しい。失敗するのは語彙が合わないだけ。

#### 推奨改善方向: 被写体リスト選択型
```
写真 + 被写体リスト → AI「この中のどれに見える？」→ 被写体→備考が一意に決まる
```

**備考 ≠ 被写体**（ここが勘所）:
- 備考は管理上の名称（「初期転圧状況」「2次転圧状況」）
- 被写体はAIが実際に見るもの（「鉄輪ローラーが転圧」「ゴムタイヤローラーが転圧」）
- AIには被写体の説明を見せて選ばせる → 選んだ被写体から備考が一意に決まる

#### 具体例: ローラー3兄弟（ドメイン知識が必要な代表例）
| 備考 | 被写体（AIが見るもの） | 視覚的特徴 |
|------|----------------------|-----------|
| 初期転圧状況 | マカダムローラー | 鉄の円筒（前後とも鉄輪） |
| 2次転圧状況 | タイヤローラー | ゴムタイヤ（前後ともゴム） |
| 仕上げ転圧状況 | コンバインドローラー | 前が鉄輪＋後がゴムタイヤ |

予備知識なしでは見分けがつかないが、被写体リストがあればAIは視覚的特徴から正しく選べる。

#### 候補リスト設計
- 全マスタ備考に対応する「被写体の説明」を新規作成（人手+AI補助）
- 全工種で約100〜200エントリ → フィルタ不要、Geminiコンテキストに余裕
- 区画線の線種判定（CoTプロンプト方式）は既にこの選択型パターンで成功している
- `common/src/step2.rs`, `common/src/prompt_format.rs` に既存のマスタ→プロンプト変換あり

### apply_station（測点一括適用）
- 安全管理写真・品質管理写真 → 測点ではなく日付（◯月◯日）を設定
- 区画線工 → スキップ（線種ごとに撮影するため固定測点は不適切）
  - 以前の-Sで誤設定された同一station値はクリア
- その他 → 指定された測点を適用

### 区画線工の線種判定（CoTプロンプト方式）
- 区画線工は線種ごとに撮影するため、写真から「何の線を引いているか」の判定が必要
- **設計書の線種リストを選択肢として与え、Chain-of-Thought（段階推論）で判定**
  - Step1: テープの線形状（直線/曲線/角度）を答えさせる
  - Step2: テープ全体の図形（直線/平行な帯/ひし形/矢印/文字）を答えさせる
  - Step3: 図形から線種を1つ選ばせる
  - CoTなしだとダイヤモンド標示を停止線/横断歩道と誤認する問題を解決
- **実装済み**: `--line-types <FILE>` CLIフラグでJSON線種リストを指定
  - JSON形式: `{"line_types": [{"name": "中央線", "length_m": 230}, ...]}`
  - 判定タイミング: 区画線工と判定された写真に対してGemini CLI 2次問い合わせ
  - 判定結果: stationフィールドに線種名を設定
  - モデル: **gemini-2.5-pro**（flash系は視覚認識精度が不足）
  - 実装: `line_type_detector.rs` の `detect_line_type()`, `build_line_type_prompt()`, `extract_line_type_from_response()`, `run_gemini_cli_for_line_type()`, `find_git_bash()`

## モジュール構成

### CLI側 (src/)
| モジュール | 役割 |
|-----------|------|
| cli.rs | clap CLI定義 |
| commands.rs | コマンドハンドラ |
| analysis.rs | パイプラインオーケストレーション（スキャン→tagger→マスタ→正規化） |
| ocr_parser.rs | detected_text正規化・キー:値抽出（KNOWN_KEYS, normalize, extract_kv） |
| master_matcher.rs | 工種マスタ照合（bigram, focus_target boost, safety判定） |
| line_type_detector.rs | 区画線の線種判定（CoTプロンプト, Gemini CLI呼び出し） |
| analyzer/ | AI解析（claude_cli.rs, cache.rs） |
| scanner/ | 画像ファイル発見 + EXIF |
| normalizer/ | 測定値統一, alias.rs |
| export/ | PDF/Excel/XML生成 |

### 共有ライブラリ (common/src/)
| モジュール | 役割 |
|-----------|------|
| types.rs | AnalysisResult, PhotoData trait |
| layout.rs | PDF/Excelレイアウト定数, FieldKey |
| hierarchy/ | 工種階層マスタCSVパース |
| analyzer.rs | AnalyzerError, 自動工種検出 |
| step2.rs | Step2照合プロンプト生成 |
| prompts.rs | PHOTO_CATEGORIES, プロンプト構築 |
| parser.rs | JSONパースユーティリティ |
| alias.rs | AliasConfig, エイリアス適用 |
| error.rs | Error（合成ハブ）, ExportError, HierarchyError |
| export/ | PDF/Excel生成（feature-gated） |

## エラー型（モジュール分散）
```
Error（error.rs: 合成ハブ）
├── Io, Json, MissingFile, InvalidFormat
├── Analyzer ← AnalyzerError (analyzer.rs)
│   ├── ResponseParse
│   └── TaggerEmpty
├── Export ← ExportError (export/mod.rs)
│   └── Failed { format, detail }
└── Hierarchy ← HierarchyError (hierarchy/mod.rs)
    ├── CsvParse { line, detail }
    └── MasterNotFound
```

## 出力形式

### PDF
- A4、2-3枚/ページ、日本語フォント埋め込み
- quality: high(1400px/85%), medium(800px/75%), low(500px/60%)
- **測定値行の自動非表示**: measurementsが空/"-"のとき、測定値行をラベルごと非表示にして7フィールドに配分

### Excel
- rust_xlsxwriter、サムネイル埋め込み、ヘッダ装飾
- PDFと同様、測定値が空のとき測定値行を非表示

### PHOTO.XML
- GASPhotoAIManager互換形式

### PDF/Excel出力後の検証手順
PDF/Excelを出力したら、必ずReadツールで開いて目視確認すること:
1. `Read`ツールでPDFの1ページ目を読む（`pages: "1"`）
2. 確認ポイント:
   - フィールド数: 測定値ありなら8行、なしなら7行
   - ラベルと値の対応が正しいか
   - レイアウト崩れがないか（写真と情報欄の配置）
   - 安全管理写真に「測定値」行が残っていないか
3. 問題があればコード修正→再出力→再確認

## 工種マスタ
- `master/construction_hierarchy.csv` — 共通エントリ（安全管理写真・現場管理費）
- `master/by_work_type/` — 工種別CSV（舗装工, 区画線工, 仮設工, 道路付属施設工, etc.）
- 全工種モード: 全by_work_typeファイル + 共通CSVをマージ読み込み（`HierarchyMaster::from_csv_files`）
- CSV列: `費目, 写真区分, 工種, 種別, 細別, 備考, 検索パターン`

## キャッシュ
- 場所: `<photo_folder>/.photo-ai-cache.json`
- SHA-256ハッシュ → AnalysisResult
- `--use-cache` で増分解析

## 除外フォルダ（デフォルトスキップ）
- 非使用, hisiyou, 不要, excluded
- `--include-all` で含める

## Ground Truthテスト
- `tests/ground_truth/0211_cutting.json`: 14枚分の正解データ（0211切削・施工状況）
- `tests/ground_truth_test.rs`: result JSONとGTの比較テスト
- result JSONが存在しない環境ではスキップ
- `cargo test gt_` で実行
- 回帰検出: 解析パイプラインの変更で既存の正解が壊れたら即座に検出

## 出来形管理用紙の測定値読み取り

### 車線の判定ロジック
- 出来形管理用紙の「幅員」欄で、**実測値が記入されている側**が当該施工車線
- 例: 左幅員の実測が記入済み・右幅員の実測が空欄 → **左車線**の出来形管理
- 同一測点の全3枚（全景・計測・管理用紙アップ）に同じmeasurementsを入れる

### measurements統一フォーマット（管理用紙アップ写真用）

左車線の場合:
```
左車線　切削基準高 幅員W1
設計: V1=9.819 V2=9.842 V3=9.861
実施: V1=9.815 V2=9.842 V3=9.860
幅員W1 設計: 4.20 実測: 4.20
```

右車線の場合:
```
右車線　切削基準高 幅員W2
設計: V4=10.001 V5=9.974
実施: V4=10.000 V5=9.970
幅員W2 設計: 3.18 実測: 3.18
```

ルール:
- 4行: ヘッダー + 設計値 + 実施値 + 幅員
- 左車線: V1～V3 + W1（左幅員） / 右車線: V4～V5 + W2（右幅員）
- V値は `V{n}=X.XXX` 形式でスペース区切り（各点が独立値であることを明示）
- 幅員は別行に `幅員W{1or2} 設計: X.XX 実測: X.XX`
- 設計・実施の両方を必ず記録
- **切削厚(mm)は不要**（設計と実施の差で自明）
- **備考**: 路面切削工出来形測定（マスタ `舗装工.csv` L67 と一致させる）

### CLIによる自動生成（normalize --lane）
```bash
# 左車線の出来形（V1～V3 + W1を自動パース・統一）
photo-ai normalize result.json --lane left --dekigata-remarks "路面切削工出来形測定"

# 右車線の出来形（V4～V5 + W2を自動パース・統一）
photo-ai normalize result.json --lane right --dekigata-remarks "路面切削工出来形測定"
```
- `src/normalizer/dekigata.rs`: OCRパース・車線判定・フォーマット生成
- detected_textに「切削高(設計) V1=...」形式があれば自動パース
- 同一測点3枚セットに同じmeasurementsと備考を統一適用

### 管理用紙の構造（路面切削工の場合）
```
出来形管理用紙 No.X
├── 断面図: GH/FH, V1〜V5の位置, 勾配, 幅員
├── 計画高(設計): V1〜V5 の5点
├── 計画高(実施): （未記入の場合あり）
├── 切削高(設計): V1〜V5 の5点
├── 切削高(実施): V1〜V3 の3点のみ記入が多い（V4,V5は未記入）
└── 幅員: 左/右 × 設計/実測
```
※切削厚(mm)はmeasurementsに含めない（設計と実施の差で自明）

### PDF描画ルール（測定値フィールド）
- 「測定値」ラベルを省略し、セル幅フルに使う
- **実測値行（"実施"/"実測"を含む行）は赤色で描画**（設計と実測が同居する行は実測部分のみ赤）
- フォント: base 11.0pt / min 9.0pt（通常フィールドより大きめ）
- 実装: `add_measurements_text_ops()` in `src/export/pdf.rs`

### 検証観点
- detectedTextの数値とmeasurementsが一致すること
- 4測点間でmeasurementsフォーマット（改行位置・区切り文字）が統一されていること
- 写真の目視読み取りとOCR値の突合（特に手書き赤字は誤読しやすい）

### パス指定の注意（Windows）
- photo-ai-rustにはフォワードスラッシュ`/`でパスを渡すこと
- バックスラッシュ`\`だとphoto-taggerステップで無言で失敗する（exit code 1）

## 設定
- `~/.config/photo-ai/config.json`
- デフォルトAI: gemini（gemini-3-flash-preview）
