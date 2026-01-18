---
name: web-gemini-analyzer
description: Web版Gemini写真解析・写真台帳出力
status: done
created: 2026-01-18
github_issue: 32
---

# Epic: Web版Gemini写真解析

## 概要

ブラウザ上でGemini APIを使用して写真解析を行い、PDF/Excel写真台帳を出力するWebアプリケーション。

## 完了タスク

- [x] HTML+JSでWebUI作成
- [x] Gemini API連携（バッチ解析）
- [x] pdf-libでPDF写真台帳出力（React版と同一レイアウト）
- [x] SheetJSでExcel出力
- [x] GitHub Pagesデプロイ

## 機能

### 入力
- ドラッグ&ドロップで複数写真アップロード
- Gemini APIキー（ローカルストレージ保存）
- タイトル・1ページあたりの写真数設定

### 解析
- バッチ処理（5枚ずつ）
- 進捗表示
- 抽出項目: 写真区分、工種、種別、細別、測点、備考

### 出力
- PDF: A4、1ページ2〜3枚、写真+情報欄レイアウト
- Excel: 全フィールド付きテーブル

## URL

https://yuujikamura.github.io/photo-ai-rust/
