#!/bin/bash
# Re-run photo-ai analyze on all source folders, saving output to a separate directory
# Uses --use-cache to avoid re-calling Gemini (tests post-processing: master matching, normalization)

# set -e removed: ((var++)) returns 1 when var==0, killing the script

BASE="H:/マイドライブ/〇市道 南千反畑町第１号線舗装補修工事/１５工事写真"
OUT_DIR="/c/Users/yuuji/photo-ai-rust/tests/ground_truth/pipeline_output"
PHOTO_AI="/c/Users/yuuji/photo-ai-rust"
# MASTER not used — auto-loads all by_work_type CSVs

mkdir -p "$OUT_DIR"

# List of source folders (relative to BASE)
FOLDERS=(
    "0209切削/温度管理"
    "0209切削/散布量試験"
    "0209切削/施工状況"
    "0209切削/切削出来形"
    "0211切削/安全パトロール"
    "0211切削/温度管理"
    "0211切削/施工状況"
    "0211切削/切削出来形"
    "0212切削/温度管理"
    "0212切削/施工状況"
    "0212切削/切削機"
    "0212切削/切削出来形 立会"
    "0212切削/切削出来形"
    "0213切削/施工状況"
    "0213切削/温度管理"
    "0213切削/切削出来形"
    "0213切削/切削出来形/No.9"
    "Photomanager/20260209/交通保安施設配置確認"
    "Photomanager/20260209/使用機械"
    "Photomanager/20260209/社外安全パトロール"
    "Photomanager/20260209/重機始業前点検"
    "Photomanager/20260211/トラックスケール_計量状況"
    "Photomanager/20260211/交通保安施設_設置状況"
    "Photomanager/20260211/出荷指示確認"
    "Photomanager/20260211/処分状況_社内検査"
    "Photomanager/20260212/安全パトロール"
    "Photomanager/20260212/交通保安施設"
    "Photomanager/20260213/交通保安施設"
    "Photomanager/20260213/始業前点検"
)

SUCCESS=0
FAIL=0

for folder in "${FOLDERS[@]}"; do
    src="$BASE/$folder"
    # Create flat name for output
    flat=$(echo "$folder" | tr '/' '_')
    out_file="$OUT_DIR/${flat}__result.json"

    if [ ! -d "$src" ]; then
        echo "SKIP: $folder (directory not found)"
        ((FAIL++))
        continue
    fi

    echo "Running: $folder ..."
    cd "$PHOTO_AI"
    if cargo run -- analyze "$src" -o "$out_file" --use-cache 2>&1 | tail -3; then
        echo "  -> OK: $out_file"
        ((SUCCESS++))
    else
        echo "  -> FAIL: $folder"
        ((FAIL++))
    fi
    echo
done

echo "=== Done: $SUCCESS succeeded, $FAIL failed ==="
