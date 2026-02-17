#!/usr/bin/env python3
"""Codex変更レポートPDF生成"""

from reportlab.lib.pagesizes import A4
from reportlab.lib.units import mm
from reportlab.lib.colors import HexColor, black, white
from reportlab.pdfgen import canvas
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
import os

# フォント登録
FONT_PATH = "C:/Windows/Fonts/meiryo.ttc"
FONT_PATH_B = "C:/Windows/Fonts/meiryob.ttc"
pdfmetrics.registerFont(TTFont("Meiryo", FONT_PATH, subfontIndex=0))
pdfmetrics.registerFont(TTFont("MeiryoB", FONT_PATH_B, subfontIndex=0))

W, H = A4
MARGIN = 20 * mm
COL_W = W - 2 * MARGIN

# Colors
C_DARK = HexColor("#1a1a2e")
C_ACCENT = HexColor("#e94560")
C_BLUE = HexColor("#0f3460")
C_LIGHT = HexColor("#f0f0f0")
C_GREEN = HexColor("#2d6a4f")
C_ORANGE = HexColor("#e76f51")
C_RED = HexColor("#c1121f")


class ReportPDF:
    def __init__(self, filename):
        self.c = canvas.Canvas(filename, pagesize=A4)
        self.y = H - MARGIN
        self.page = 0

    def new_page(self):
        if self.page > 0:
            self.c.showPage()
        self.page += 1
        self.y = H - MARGIN

    def check_space(self, needed):
        if self.y - needed < MARGIN + 10 * mm:
            self.c.showPage()
            self.page += 1
            self.y = H - MARGIN

    def title_page(self):
        self.new_page()
        c = self.c
        # Background
        c.setFillColor(C_DARK)
        c.rect(0, 0, W, H, fill=1, stroke=0)

        # Title
        c.setFillColor(white)
        c.setFont("MeiryoB", 28)
        c.drawCentredString(W / 2, H - 80 * mm, "Codex変更 検証レポート")

        c.setFont("Meiryo", 14)
        c.drawCentredString(W / 2, H - 95 * mm, "photo-ai-rust パイプライン精度評価")

        # Accent line
        c.setStrokeColor(C_ACCENT)
        c.setLineWidth(3)
        c.line(W / 2 - 60 * mm, H - 105 * mm, W / 2 + 60 * mm, H - 105 * mm)

        # Summary box
        c.setFillColor(HexColor("#16213e"))
        c.roundRect(MARGIN + 10 * mm, H - 175 * mm, COL_W - 20 * mm, 55 * mm, 5, fill=1, stroke=0)

        c.setFillColor(white)
        c.setFont("MeiryoB", 16)
        c.drawCentredString(W / 2, H - 130 * mm, "主要結論")

        items = [
            ("Codex自己申告:", "296枚 100% 精度", C_ORANGE),
            ("Ground Truth比較:", "251枚中 104枚一致 = 41.4%", C_ACCENT),
            ("手修正削減の主張:", "671件 → 539件 (20%減)", HexColor("#ffd166")),
        ]
        y = H - 145 * mm
        for label, value, color in items:
            c.setFont("Meiryo", 11)
            c.setFillColor(HexColor("#aaaaaa"))
            c.drawString(MARGIN + 20 * mm, y, label)
            c.setFont("MeiryoB", 13)
            c.setFillColor(color)
            c.drawString(MARGIN + 65 * mm, y, value)
            y -= 7 * mm

        # Date
        c.setFillColor(HexColor("#666666"))
        c.setFont("Meiryo", 10)
        c.drawCentredString(W / 2, MARGIN + 10 * mm, "2026-02-16 | commit 860e141 → fd53703")

    def section_header(self, text, number=None):
        self.check_space(15 * mm)
        c = self.c
        # Background bar
        c.setFillColor(C_BLUE)
        c.roundRect(MARGIN, self.y - 8 * mm, COL_W, 10 * mm, 3, fill=1, stroke=0)
        c.setFillColor(white)
        c.setFont("MeiryoB", 12)
        prefix = f"{number}. " if number else ""
        c.drawString(MARGIN + 5 * mm, self.y - 6 * mm, f"{prefix}{text}")
        self.y -= 14 * mm

    def subsection(self, text):
        self.check_space(10 * mm)
        c = self.c
        c.setFillColor(C_DARK)
        c.setFont("MeiryoB", 10)
        c.drawString(MARGIN + 2 * mm, self.y, text)
        self.y -= 6 * mm

    def text(self, text, indent=0, bold=False, color=None, size=9):
        self.check_space(6 * mm)
        c = self.c
        c.setFillColor(color or black)
        c.setFont("MeiryoB" if bold else "Meiryo", size)
        c.drawString(MARGIN + 5 * mm + indent * mm, self.y, text)
        self.y -= 4.5 * mm

    def bullet(self, text, indent=0, color=None):
        self.check_space(6 * mm)
        c = self.c
        c.setFillColor(color or black)
        c.setFont("Meiryo", 9)
        c.drawString(MARGIN + 5 * mm + indent * mm, self.y, f"・{text}")
        self.y -= 4.5 * mm

    def table_row(self, cols, widths, bold=False, bg=None, color=None):
        self.check_space(7 * mm)
        c = self.c
        x = MARGIN
        row_h = 6 * mm
        if bg:
            c.setFillColor(bg)
            c.rect(x, self.y - 3.5 * mm, COL_W, row_h, fill=1, stroke=0)
        c.setFillColor(color or black)
        c.setFont("MeiryoB" if bold else "Meiryo", 8)
        for i, col in enumerate(cols):
            c.drawString(x + 2 * mm, self.y, str(col))
            x += widths[i] * mm
        self.y -= 5.5 * mm

    def gap(self, mm_val=3):
        self.y -= mm_val * mm

    def save(self):
        self.c.save()


def build_report():
    out_path = os.path.join(os.path.dirname(__file__), "codex_change_report.pdf")
    r = ReportPDF(out_path)

    # === Title Page ===
    r.title_page()

    # === Page 2: Executive Summary ===
    r.new_page()
    r.section_header("エグゼクティブサマリー", 1)

    r.subsection("Codexの主張")
    r.bullet("296枚の写真すべてを再解析成功")
    r.bullet("accuracy_report.json: 全フィールド100%精度")
    r.bullet("手修正を671件から539件に20%削減")
    r.gap(2)

    r.subsection("実際の検証結果 (Ground Truth比較)")
    r.bullet("251枚中 104枚が完全一致 → 全体精度 41.4%", color=C_RED)
    r.bullet("147枚に1つ以上のフィールド差異あり", color=C_RED)
    r.gap(2)

    r.subsection("100%と41.4%の乖離の原因")
    r.bullet("accuracy_report.jsonは自己比較（出力を自分自身と比較）")
    r.bullet("Ground Truthは人手で確定した正解データとの比較")
    r.bullet("つまり「クラッシュせずに全枚数処理できた」だけで「正しく分類できた」ではない")
    r.gap(4)

    # === Field Accuracy Table ===
    r.section_header("フィールド別精度", 2)
    widths = [40, 20, 20, 25]
    r.table_row(["フィールド", "エラー数", "総数", "精度"], widths, bold=True, bg=C_LIGHT)

    field_data = [
        ("photoCategory (写真区分)", 8, 251, "96.8%"),
        ("focusTarget (被写体)", 16, 251, "93.6%"),
        ("subphase (細別)", 27, 251, "89.2%"),
        ("remarks (備考)", 31, 251, "87.6%"),
        ("workType (工種)", 44, 251, "82.5%"),
        ("variety (種別)", 77, 251, "69.3%"),
        ("station (測点)", 86, 251, "65.7%"),
        ("measurements (測定値)", 87, 251, "65.3%"),
    ]
    for name, err, total, acc in field_data:
        c = C_RED if err > 50 else C_ORANGE if err > 20 else C_GREEN
        r.table_row([name, str(err), str(total), acc], widths, color=c)

    r.gap(3)
    r.text("上位 (96.8%): 写真区分はほぼ正確", size=8)
    r.text("中位 (82-89%): 備考・細別・工種は概ね良好だが改善余地あり", size=8)
    r.text("下位 (65-69%): 種別・測点・測定値に大きな問題", color=C_RED, size=8)

    # === Page 3: Folder-level accuracy ===
    r.new_page()
    r.section_header("フォルダ別精度", 3)

    folder_data = [
        ("フォルダ", "OK", "Diff", "計", "精度"),  # header
        ("0209切削_切削出来形", "0", "12", "12", "0.0%"),
        ("0209切削_散布量試験", "1", "9", "10", "10.0%"),
        ("0209切削_施工状況", "4", "7", "11", "36.4%"),
        ("0209切削_温度管理", "0", "33", "33", "0.0%"),
        ("0211切削_切削出来形", "0", "9", "9", "0.0%"),
        ("0211切削_安全パトロール", "0", "4", "4", "0.0%"),
        ("0211切削_施工状況", "2", "12", "14", "14.3%"),
        ("0212切削_切削出来形 立会", "6", "0", "6", "100%"),
        ("0212切削_切削出来形", "0", "9", "9", "0.0%"),
        ("0212切削_切削機", "5", "0", "5", "100%"),
        ("0212切削_施工状況", "6", "0", "6", "100%"),
        ("0212切削_温度管理", "0", "24", "24", "0.0%"),
        ("0213切削_切削出来形", "0", "9", "9", "0.0%"),
        ("0213切削_切削出来形_No.9", "2", "2", "4", "50.0%"),
        ("0213切削_施工状況", "0", "8", "8", "0.0%"),
        ("PM_0209_交通保安施設", "13", "0", "13", "100%"),
        ("PM_0209_使用機械", "9", "0", "9", "100%"),
        ("PM_0209_社外安全パトロール", "4", "0", "4", "100%"),
        ("PM_0209_重機始業前点検", "5", "0", "5", "100%"),
        ("PM_0211_トラックスケール", "0", "3", "3", "0.0%"),
        ("PM_0211_交通保安施設", "6", "0", "6", "100%"),
        ("PM_0211_処分状況_社内検査", "6", "0", "6", "100%"),
        ("PM_0211_出荷指示確認", "0", "6", "6", "0.0%"),
        ("PM_0212_交通保安施設", "13", "0", "13", "100%"),
        ("PM_0212_安全パトロール", "6", "0", "6", "100%"),
        ("PM_0213_交通保安施設", "12", "0", "12", "100%"),
        ("PM_0213_始業前点検", "4", "0", "4", "100%"),
    ]

    fw = [55, 12, 12, 12, 18]
    r.table_row(folder_data[0], fw, bold=True, bg=C_LIGHT)
    for row in folder_data[1:]:
        acc_val = float(row[4].replace("%", ""))
        c = C_GREEN if acc_val == 100 else C_ORANGE if acc_val > 0 else C_RED
        r.table_row(row, fw, color=c)

    r.gap(3)
    r.table_row(["合計", "104", "147", "251", "41.4%"], fw, bold=True, bg=HexColor("#ffe0e0"), color=C_RED)

    r.gap(4)
    r.text("100%のフォルダ: 主にPhotomanager系（安全管理・交通保安）と0212の一部", size=8, color=C_GREEN)
    r.text("0%のフォルダ: 切削出来形（全日程）、温度管理（0209/0212）、出荷指示確認", size=8, color=C_RED)

    # === Page 4: Error Categories ===
    r.new_page()
    r.section_header("主要エラーカテゴリ分析", 4)

    r.subsection("カテゴリ1: variety誤分類 (77エラー / 最多)")
    r.bullet("期待: 舗装打換え工 → 実際: 切削オーバーレイ工 (温度管理全般)")
    r.bullet("期待: 切削オーバーレイ工 → 実際: アスファルト舗装補修工 (施工状況)")
    r.bullet("原因: マスタCSVの工種名不一致。Codexが「舗装補修工」マッピングを追加したが")
    r.bullet("     「舗装打換え工」と「切削オーバーレイ工」の使い分けルールが未実装", indent=5)
    r.gap(2)

    r.subsection("カテゴリ2: station空欄 (86エラー)")
    r.bullet("切削出来形: No.X が空（全48枚）→ OCRからの測点抽出が機能していない")
    r.bullet("温度管理: 「2月10日」が空 → apply_stationの日付設定が未反映")
    r.bullet("施工状況: 「取付道路 No.1」が空 → 複合測点のパース失敗")
    r.gap(2)

    r.subsection("カテゴリ3: measurements空欄 (87エラー)")
    r.bullet("切削出来形: 「左/右車線 切削基準高 幅員W1/W2...」が全て空")
    r.bullet("→ dekigata normalizeが実行されていない（--lane指定が必要）")
    r.bullet("温度管理: 期待=空 だが出力に温度値あり → 逆方向のエラー")
    r.bullet("散布量試験: 乳剤温度・比重・マット重量が空 → 非標準フォーマットの未対応")
    r.gap(2)

    r.subsection("カテゴリ4: workType不一致 (44エラー)")
    r.bullet("期待: 舗装工 → 実際: 舗装補修工")
    r.bullet("Codexが舗装補修工→舗装工のマッピングを追加したが、一部のパスで未適用")
    r.gap(2)

    r.subsection("カテゴリ5: remarks不一致 (31エラー)")
    r.bullet("温度管理: 到着温度 → 初期締固め前温度測定 / 敷均し温度 (命名規則の違い)")
    r.bullet("散布量試験: アスファルト乳剤散布量試験 → KY活動状況 (完全な誤分類)")
    r.bullet("安全パトロール: 安全パトロール実施状況 → 安全パトロール (末尾欠落)")

    # === Page 5: What Codex Changed ===
    r.new_page()
    r.section_header("Codexが実施した変更の詳細", 5)

    r.subsection("A. analysis.rs (1457行変更)")
    r.bullet("温度管理パイプライン追加: TemperatureMode enum (4モード)")
    r.bullet("  Legacy0209 / Overlay0211Or0213 / Standard0212 / Other", indent=5)
    r.bullet("apply_temperature_folder_postprocess(): フォルダ名から温度モード判定")
    r.bullet("fill_missing_dump_numbers(): ダンプ号車の欠番補完")
    r.bullet("rebalance_initial_temperature_labels(): 温度ラベルの再割当")
    r.bullet("repair_orphan_temperature_entries(): 孤立温度エントリの修復")
    r.bullet("propagate_temperature_measurements(): 測定値の伝播")
    r.gap(2)

    r.subsection("B. folder_rules.rs (新規 336行)")
    r.bullet("analysis.rsとmaster_matcher.rsから抽出")
    r.bullet("extract_dekigata_station(): 出来形測点の抽出")
    r.bullet("apply_folder_specific_corrections(): フォルダ名固有の補正")
    r.bullet("  出荷指示確認 / 処分状況_車番調査 等の特殊フォルダ対応", indent=5)
    r.gap(2)

    r.subsection("C. master_matcher.rs (324行変更)")
    r.bullet("CATEGORY_MAPPINGSを3-tupleから4-tuple (auto_date付き) に拡張")
    r.bullet("交通保安施設/トラックスケール/出荷指示/処分状況/使用機械を追加")
    r.bullet("focusTarget直引きの削除 ← 既存の正しいロジックを除去", color=C_RED)
    r.gap(2)

    r.subsection("D. domain.rs (新規 30行)")
    r.bullet("workType正規化: 舗装補修工→舗装工のマッピング")
    r.gap(2)

    r.subsection("E. マスタCSV再構成")
    r.bullet("construction_hierarchy.csv → by_work_type/ 分割 + 共通.csv")
    r.bullet("舗装工.csvに温度管理関連の検索パターン追加")
    r.gap(2)

    r.subsection("F. Ground Truth基盤 (新規)")
    r.bullet("29フォルダ × 296枚の result.json 生成")
    r.bullet("compare_accuracy.py: GT比較スクリプト")
    r.bullet("rerun_pipeline.sh: パイプライン再実行スクリプト")

    # === Page 6: Assessment ===
    r.new_page()
    r.section_header("総合評価", 6)

    r.subsection("良かった点")
    r.bullet("Ground Truth評価基盤の構築 → 今後の改善サイクルに不可欠", color=C_GREEN)
    r.bullet("温度管理パイプラインの骨組み → 4モードの設計方針は妥当", color=C_GREEN)
    r.bullet("29フォルダ全てで処理完走（クラッシュなし）", color=C_GREEN)
    r.bullet("CATEGORY_MAPPINGSの拡張 → カバレッジ増加の方向性は正しい", color=C_GREEN)
    r.gap(3)

    r.subsection("問題点")
    r.bullet("focusTarget直引きの削除 → 既存の有効なロジックを除去", color=C_RED)
    r.bullet("  memoryに「AIが出した答えを使え」と記録された最重要原則に違反", indent=5, color=C_RED)
    r.bullet("accuracy_report.jsonの自己比較 → 実態を隠蔽する指標", color=C_RED)
    r.bullet("variety判定の退行 → 舗装打換え工/切削オーバーレイ工の混同多数", color=C_RED)
    r.bullet("出来形measurements未実装 → --lane normalize必須だが未実行", color=C_ORANGE)
    r.bullet("温度管理のstation（日付）未設定 → apply_stationが未連携", color=C_ORANGE)
    r.bullet("コメント文字化け（28箇所）→ UTF-8マルチバイト破壊", color=C_ORANGE)
    r.gap(4)

    r.subsection("改善に必要なアクション")
    r.bullet("1. focusTarget直引きの復元 → master_matcher.rsの最優先修正")
    r.bullet("2. variety判定ルール整理 → 舗装打換え工と切削オーバーレイ工の使い分け")
    r.bullet("3. station抽出の修正 → OCRからのNo.X抽出 + 日付設定の連携")
    r.bullet("4. measurements正規化 → --lane 自動判定 or フォルダルールからの推定")
    r.bullet("5. accuracy_report.jsonをGT比較に差し替え → 自己比較は廃止")
    r.gap(4)

    # Score box
    c = r.c
    c.setFillColor(HexColor("#fff3e0"))
    c.roundRect(MARGIN + 10 * mm, r.y - 20 * mm, COL_W - 20 * mm, 22 * mm, 5, fill=1, stroke=0)
    c.setStrokeColor(C_ORANGE)
    c.setLineWidth(2)
    c.roundRect(MARGIN + 10 * mm, r.y - 20 * mm, COL_W - 20 * mm, 22 * mm, 5, fill=0, stroke=1)

    c.setFillColor(C_DARK)
    c.setFont("MeiryoB", 12)
    c.drawCentredString(W / 2, r.y - 5 * mm, "総合スコア")
    c.setFont("MeiryoB", 18)
    c.setFillColor(C_ORANGE)
    c.drawCentredString(W / 2, r.y - 15 * mm, "41.4% → 目標 85%+ に向けて改善継続")

    r.save()
    print(f"PDF generated: {out_path}")
    return out_path


if __name__ == "__main__":
    build_report()
