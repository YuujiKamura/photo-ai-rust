"""着手前竣工写真帳PDF生成（JPEG直接埋め込み）"""
import sys, os, re
from pathlib import Path
from fpdf import FPDF

# レイアウト定数 (mm)
PAGE_W, PAGE_H = 297, 210
MARGIN_X = 6.8   # 左右
MARGIN_TOP = 10  # 上
PHOTO_BOTTOM = 25.4  # 写真下端（ページ底からの距離）

# キャプションY座標（ページ上端から）
STATION_Y = PAGE_H - 19.2
RULE1_Y   = PAGE_H - 16.7
LABEL_Y   = PAGE_H - 12.8
RULE2_Y   = PAGE_H - 10.0

# 罫線
RULE_W = 93  # mm (264pt≒93mm)

# フォント
TITLE_SIZE = 32
PROJECT_NAME_SIZE = 24
STATION_FONT_SIZE = 13.5 * 0.3528  # pt→mm相当だがfpdfはptで指定
LABEL_FONT_SIZE = 11.5 * 0.3528

FONT_PATH_DEMIBOLD = "C:/Windows/Fonts/yumindb.ttf"
FONT_PATH_LIGHT = "C:/Windows/Fonts/yuminl.ttf"


def find_before_after(d: Path):
    """フォルダ内からbefore/after写真を検出"""
    before = after = None
    for f in sorted(d.iterdir()):
        if not f.is_file():
            continue
        low = f.name.lower()
        if not low.endswith(('.jpg', '.jpeg', '.png')):
            continue
        if 'before' in low or f.suffix == '.JPG':
            if before is None:
                before = f
        elif 'after' in low or f.suffix == '.jpg':
            if after is None:
                after = f
    return before, after


def scan_pair_folders(folder: Path):
    """P{NN}_{測点名}フォルダをスキャンしてペアリスト返却"""
    pairs = []
    pat = re.compile(r'^P(\d+)_(.+)$')
    for d in sorted(folder.iterdir()):
        if not d.is_dir():
            continue
        m = pat.match(d.name)
        if not m:
            continue
        num = int(m.group(1))
        station = m.group(2)
        before, after = find_before_after(d)
        if before and after:
            pairs.append((num, station, before, after))
        else:
            print(f"警告: {d.name} - {'着手前' if not before else '竣工'}写真が見つかりません", file=sys.stderr)
    pairs.sort(key=lambda x: x[0])
    return pairs


def add_rule(pdf, y):
    """中央揃えの水平罫線"""
    x1 = (PAGE_W - RULE_W) / 2
    pdf.set_draw_color(77, 77, 77)
    pdf.set_line_width(0.18)
    pdf.line(x1, y, x1 + RULE_W, y)


def add_photo_page(pdf, image_path, station_name, label, title_font, body_font):
    """写真ページを追加"""
    pdf.add_page(orientation='L')

    # 写真領域
    photo_x = MARGIN_X
    photo_y = MARGIN_TOP
    photo_w = PAGE_W - MARGIN_X * 2
    photo_h = PAGE_H - MARGIN_TOP - PHOTO_BOTTOM

    # 画像のアスペクト比を保ってフィット
    from PIL import Image
    with Image.open(image_path) as img:
        iw, ih = img.size
    img_aspect = iw / ih
    box_aspect = photo_w / photo_h

    if img_aspect > box_aspect:
        draw_w = photo_w
        draw_h = photo_w / img_aspect
    else:
        draw_h = photo_h
        draw_w = photo_h * img_aspect

    draw_x = photo_x + (photo_w - draw_w) / 2
    draw_y = photo_y + (photo_h - draw_h) / 2

    pdf.image(str(image_path), draw_x, draw_y, draw_w, draw_h)

    # キャプション
    pdf.set_font(title_font, size=13.5)
    pdf.set_text_color(0, 0, 0)
    tw = pdf.get_string_width(station_name)
    pdf.text((PAGE_W - tw) / 2, STATION_Y, station_name)

    add_rule(pdf, RULE1_Y)

    pdf.set_font(body_font, size=11.5)
    tw = pdf.get_string_width(label)
    pdf.text((PAGE_W - tw) / 2, LABEL_Y, label)

    add_rule(pdf, RULE2_Y)


def generate(folder, project_name, output_path):
    pairs = scan_pair_folders(Path(folder))
    print(f"ペア数: {len(pairs)}")
    if not pairs:
        print("エラー: ペアが見つかりません", file=sys.stderr)
        sys.exit(1)

    pdf = FPDF(orientation='L', unit='mm', format='A4')
    pdf.set_auto_page_break(auto=False)

    # フォント登録（fpdf2 v2.5.1+ ではuni不要）
    has_demibold = os.path.exists(FONT_PATH_DEMIBOLD)
    has_light = os.path.exists(FONT_PATH_LIGHT)
    if has_demibold:
        pdf.add_font("YuMinDemibold", "", FONT_PATH_DEMIBOLD)
    if has_light:
        pdf.add_font("YuMinLight", "", FONT_PATH_LIGHT)
    # フォールバック決定
    title_font = "YuMinDemibold" if has_demibold else ("YuMinLight" if has_light else "Helvetica")
    body_font = "YuMinLight" if has_light else title_font

    # 表紙
    pdf.add_page(orientation='L')
    pdf.set_font(title_font, size=TITLE_SIZE)
    title = "着手前-竣工写真"
    tw = pdf.get_string_width(title)
    pdf.text((PAGE_W - tw) / 2, PAGE_H / 2 - 5, title)

    pdf.set_font(title_font, size=PROJECT_NAME_SIZE)
    tw = pdf.get_string_width(project_name)
    pdf.text((PAGE_W - tw) / 2, PAGE_H / 2 + 15, project_name)

    # 各ペア
    for i, (num, station, before, after) in enumerate(pairs):
        print(f"[{i+1}/{len(pairs)}] {station}", file=sys.stderr)
        add_photo_page(pdf, before, station, "着手前", title_font, body_font)
        add_photo_page(pdf, after, station, "竣工", title_font, body_font)

    # 出力
    out = Path(output_path)
    out.parent.mkdir(parents=True, exist_ok=True)
    pdf.output(str(out))
    size_mb = out.stat().st_size / 1_048_576
    print(f"PDF出力: {out} ({size_mb:.1f}MB)")


if __name__ == "__main__":
    if len(sys.argv) < 3:
        print(f"Usage: {sys.argv[0]} <folder> <project_name> [output_path]")
        sys.exit(1)
    folder = sys.argv[1]
    project_name = sys.argv[2]
    output = sys.argv[3] if len(sys.argv) > 3 else str(
        Path(folder).parent / "写真帳まとめ" / f"着手前竣工_{project_name}.pdf"
    )
    generate(folder, project_name, output)
