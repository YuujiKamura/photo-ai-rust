"""コンタクトシート方式ペアリング精度テスト

Before/After両方をそれぞれ番号付きグリッド画像にまとめ、
1コール1問で「B{nn}と同じ場所はA何番か」を答えさせる。
"""
import os, json, subprocess, sys, re
from PIL import Image, ImageDraw, ImageFont

THUMB_DIR = 'C:/Users/yuuji/photo-ai-rust/pairing_thumbs'
GT_PATH = 'C:/Users/yuuji/photo-ai-rust/pairing_manual.json'
OUT_DIR = 'C:/Users/yuuji/photo-ai-rust/pairing_thumbs'

CELL_W, CELL_H = 267, 200
LABEL_H = 22
GAP = 4

def make_contact_sheet(image_files, prefix, cols, out_path):
    """番号付きコンタクトシートを生成。番号→ファイル名マッピングを返す。"""
    n = len(image_files)
    rows = (n + cols - 1) // cols
    sheet_w = cols * CELL_W + (cols - 1) * GAP
    sheet_h = rows * (CELL_H + LABEL_H) + (rows - 1) * GAP
    sheet = Image.new('RGB', (sheet_w, sheet_h), (255, 255, 255))
    draw = ImageDraw.Draw(sheet)

    try:
        font = ImageFont.truetype("arial.ttf", 16)
    except:
        font = ImageFont.load_default()

    mapping = {}
    for i, fpath in enumerate(image_files):
        num = i + 1
        label = f"{prefix}{num:02d}"
        mapping[num] = os.path.basename(fpath)

        col = i % cols
        row = i // cols
        x = col * (CELL_W + GAP)
        y = row * (CELL_H + LABEL_H + GAP)

        # ラベル帯
        draw.rectangle([x, y, x + CELL_W, y + LABEL_H], fill=(0, 0, 0))
        draw.text((x + 4, y + 2), label, fill=(255, 255, 255), font=font)

        # サムネイル
        try:
            thumb = Image.open(fpath)
            thumb = thumb.resize((CELL_W, CELL_H), Image.LANCZOS)
            sheet.paste(thumb, (x, y + LABEL_H))
        except Exception as e:
            print(f"Warning: cannot load {fpath}: {e}", file=sys.stderr)

    sheet.save(out_path, quality=90)
    print(f"Generated: {out_path} ({sheet_w}x{sheet_h}, {n} images)")
    return mapping


def main():
    gt = json.load(open(GT_PATH, encoding='utf-8'))
    test_pages = [r for r in gt if r['before_file'] and r['after_file']]

    # Before/Afterファイル一覧（ソート順のまま）
    before_files = sorted([
        os.path.join(THUMB_DIR, f) for f in os.listdir(THUMB_DIR)
        if f.startswith('before_')
    ])
    after_files = sorted([
        os.path.join(THUMB_DIR, f) for f in os.listdir(THUMB_DIR)
        if f.startswith('after_')
    ])

    print(f"Before: {len(before_files)} files, After: {len(after_files)} files")
    print(f"GT pairs: {len(test_pages)}")

    # コンタクトシート生成
    before_sheet = os.path.join(OUT_DIR, 'contact_before.jpg')
    after_sheet = os.path.join(OUT_DIR, 'contact_after.jpg')

    before_map = make_contact_sheet(before_files, 'B', 5, before_sheet)  # 5列×4行
    after_map = make_contact_sheet(after_files, 'A', 7, after_sheet)     # 7列×3行

    # 逆引き: before filename -> B番号, after filename -> A番号
    before_fname_to_num = {}
    for num, fname in before_map.items():
        # before_P01.jpg -> P01
        before_fname_to_num[fname] = num

    after_fname_to_num = {}
    for num, fname in after_map.items():
        # after_20260223_125812.jpg -> 20260223_125812.jpg
        after_fname_to_num[fname.replace('after_', '')] = num

    prompt_template = (
        "Image 1 is a numbered grid of BEFORE-construction road photos (B01-B{before_max}).\n"
        "Image 2 is a numbered grid of AFTER-construction road photos (A01-A{after_max}).\n"
        "Which AFTER number (A01-A{after_max}) shows the SAME road location as {query}?\n"
        "Match by: vanishing point direction, building silhouettes, lane markings.\n"
        "Output ONLY the number like A07."
    )

    results = []
    for r in test_pages:
        page = r['before_page']
        before_thumb_name = f'before_P{page:02d}.jpg'
        if before_thumb_name not in before_fname_to_num:
            print(f"P{page:02d} skip (no before thumbnail)")
            continue

        b_num = before_fname_to_num[before_thumb_name]
        gt_a_num = after_fname_to_num.get(r['after_file'])
        query_label = f"B{b_num:02d}"

        prompt = prompt_template.format(
            before_max=len(before_map),
            after_max=len(after_map),
            query=query_label,
        )

        stdin_text = f"@{before_sheet} @{after_sheet} {prompt}"

        try:
            proc = subprocess.run(
                'gemini -m gemini-3-flash-preview --yolo -o text',
                input=stdin_text, capture_output=True, text=True,
                timeout=300, shell=True
            )
            answer = proc.stdout.strip()
        except subprocess.TimeoutExpired:
            answer = "TIMEOUT"
        except Exception as e:
            answer = f"ERROR: {e}"

        # 回答からA番号を抽出
        m = re.search(r'A\s*(\d+)', answer, re.IGNORECASE)
        pred_num = int(m.group(1)) if m else None

        correct = pred_num == gt_a_num
        results.append({
            'page': page,
            'query': query_label,
            'gt_a': f"A{gt_a_num:02d}" if gt_a_num else '?',
            'pred_a': f"A{pred_num:02d}" if pred_num else answer[:40],
            'correct': correct,
        })
        mark = 'o' if correct else 'x'
        gt_label = f"A{gt_a_num:02d}" if gt_a_num else '?'
        pred_label = f"A{pred_num:02d}" if pred_num else answer[:40]
        print(f"{query_label} {mark}  gt={gt_label}  pred={pred_label}", flush=True)

    # 集計
    n_correct = sum(1 for r in results if r['correct'])
    n_total = len(results)
    acc = n_correct / n_total * 100 if n_total else 0
    print(f"\nAccuracy: {acc:.0f}% ({n_correct}/{n_total})")
    print(f"Baseline (individual files): 10%")


if __name__ == '__main__':
    main()
