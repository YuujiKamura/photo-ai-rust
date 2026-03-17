"""サムネイルペアリング精度テスト"""
import os, json, subprocess, sys

thumb_dir = 'C:/Users/yuuji/photo-ai-rust/pairing_thumbs'
gt = json.load(open('C:/Users/yuuji/photo-ai-rust/pairing_manual.json', encoding='utf-8'))

after_thumbs = sorted([f for f in os.listdir(thumb_dir) if f.startswith('after_')])
after_names = [f.replace('after_', '') for f in after_thumbs]
after_list = ', '.join(after_names)

test_pages = [r for r in gt if r['before_file'] and r['after_file']]

results = []
for r in test_pages:
    page = r['before_page']
    before_thumb = os.path.join(thumb_dir, f'before_P{page:02d}.jpg')
    if not os.path.exists(before_thumb):
        continue

    images = [before_thumb] + [os.path.join(thumb_dir, f) for f in after_thumbs]
    file_refs = ' '.join(['@' + p.replace('\\', '/') for p in images])
    prompt = (
        f'The first image is a BEFORE-construction photo. '
        f'The remaining 21 images are AFTER-construction candidates: [{after_list}]. '
        f'Which AFTER photo shows the SAME road location? '
        f'Match by: (1) lane marking geometry, (2) background building silhouettes. '
        f'Output ONLY the filename, nothing else.'
    )
    stdin_text = f'{file_refs} {prompt}'

    proc = subprocess.run(
        'gemini --yolo -o text -m gemini-3-flash-preview',
        input=stdin_text, capture_output=True, text=True, timeout=300, shell=True
    )
    answer = proc.stdout.strip()

    matched = None
    for name in after_names:
        if name in answer:
            matched = name
            break

    correct = matched == r['after_file']
    results.append({'page': page, 'gt': r['after_file'], 'pred': matched, 'correct': correct})
    mark = 'o' if correct else 'x'
    print(f'P{page:02d} {mark} gt={r["after_file"]} pred={matched or answer[:60]}', flush=True)

acc = sum(1 for r in results if r['correct']) / len(results) * 100
print(f'\nAccuracy: {acc:.0f}% ({sum(1 for r in results if r["correct"])}/{len(results)})')
