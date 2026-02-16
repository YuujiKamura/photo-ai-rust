#!/usr/bin/env python3
"""Compare pipeline output against ground truth result.json files.

Usage:
    python compare_accuracy.py <ground_truth_dir> <pipeline_output_dir>

Both directories should contain files named like:
    0209切削_温度管理__result.json
    Photomanager_20260209_使用機械__result.json
"""

import json
import sys
import os
import io
from pathlib import Path
from collections import defaultdict

COMPARE_FIELDS = [
    "photoCategory",
    "remarks",
    "station",
    "workType",
    "variety",
    "subphase",
    "measurements",
    "focusTarget",
]

def load_results(path):
    with open(path, "r", encoding="utf-8") as f:
        data = json.load(f)
    # Index by fileName for stable comparison
    return {item["fileName"]: item for item in data}

def normalize_value(v):
    """Normalize for comparison: strip whitespace, normalize newlines."""
    if v is None:
        return ""
    return str(v).strip().replace("\r\n", "\n").replace("\r", "\n")

def compare_files(gt_path, pipe_path):
    gt = load_results(gt_path)
    pipe = load_results(pipe_path)

    results = []
    for fname in gt:
        if fname not in pipe:
            results.append({
                "fileName": fname,
                "status": "MISSING",
                "diffs": [],
            })
            continue

        gt_item = gt[fname]
        pipe_item = pipe[fname]
        diffs = []
        for field in COMPARE_FIELDS:
            gt_val = normalize_value(gt_item.get(field, ""))
            pipe_val = normalize_value(pipe_item.get(field, ""))
            if gt_val != pipe_val:
                diffs.append({
                    "field": field,
                    "expected": gt_val,
                    "got": pipe_val,
                })
        results.append({
            "fileName": fname,
            "status": "DIFF" if diffs else "OK",
            "diffs": diffs,
        })

    return results

def main():
    if len(sys.argv) < 3:
        print(f"Usage: {sys.argv[0]} <ground_truth_dir> <pipeline_output_dir>")
        sys.exit(1)

    gt_dir = Path(sys.argv[1])
    pipe_dir = Path(sys.argv[2])

    total_photos = 0
    total_ok = 0
    total_diff = 0
    total_missing = 0
    field_errors = defaultdict(int)
    field_total = defaultdict(int)
    folder_results = []

    gt_files = sorted(gt_dir.glob("*__result.json"))
    if not gt_files:
        print(f"No ground truth files found in {gt_dir}")
        sys.exit(1)

    for gt_file in gt_files:
        name = gt_file.name
        pipe_file = pipe_dir / name
        folder_label = name.replace("__result.json", "")

        if not pipe_file.exists():
            print(f"SKIP {folder_label}: no pipeline output")
            continue

        results = compare_files(gt_file, pipe_file)
        ok = sum(1 for r in results if r["status"] == "OK")
        diff = sum(1 for r in results if r["status"] == "DIFF")
        missing = sum(1 for r in results if r["status"] == "MISSING")
        count = len(results)

        total_photos += count
        total_ok += ok
        total_diff += diff
        total_missing += missing

        for r in results:
            for d in r["diffs"]:
                field_errors[d["field"]] += 1
            for field in COMPARE_FIELDS:
                field_total[field] += 1

        folder_results.append({
            "folder": folder_label,
            "total": count,
            "ok": ok,
            "diff": diff,
            "missing": missing,
            "details": [r for r in results if r["status"] != "OK"],
        })

    # Print summary
    print("=" * 80)
    print("PIPELINE ACCURACY REPORT")
    print("=" * 80)
    print()

    # Per-folder summary
    print(f"{'Folder':<50} {'OK':>4} {'Diff':>4} {'Total':>5} {'Acc':>6}")
    print("-" * 70)
    for fr in folder_results:
        acc = fr["ok"] / fr["total"] * 100 if fr["total"] > 0 else 0
        marker = " *" if fr["diff"] > 0 else ""
        print(f"{fr['folder']:<50} {fr['ok']:>4} {fr['diff']:>4} {fr['total']:>5} {acc:>5.1f}%{marker}")

    print("-" * 70)
    overall_acc = total_ok / total_photos * 100 if total_photos > 0 else 0
    print(f"{'TOTAL':<50} {total_ok:>4} {total_diff:>4} {total_photos:>5} {overall_acc:>5.1f}%")
    print()

    # Field-level accuracy
    print("FIELD-LEVEL ACCURACY:")
    print(f"  {'Field':<20} {'Errors':>6} {'Total':>6} {'Acc':>7}")
    print("  " + "-" * 42)
    for field in COMPARE_FIELDS:
        errs = field_errors[field]
        total = field_total[field]
        acc = (total - errs) / total * 100 if total > 0 else 0
        print(f"  {field:<20} {errs:>6} {total:>6} {acc:>6.1f}%")
    print()

    # Detailed diffs
    if any(fr["details"] for fr in folder_results):
        print("DETAILED DIFFS:")
        print("-" * 80)
        for fr in folder_results:
            if not fr["details"]:
                continue
            print(f"\n[{fr['folder']}]")
            for r in fr["details"]:
                if r["status"] == "MISSING":
                    print(f"  {r['fileName']}: MISSING in pipeline output")
                else:
                    print(f"  {r['fileName']}:")
                    for d in r["diffs"]:
                        exp = d["expected"][:60] if d["expected"] else "(empty)"
                        got = d["got"][:60] if d["got"] else "(empty)"
                        print(f"    {d['field']}: expected={exp}")
                        print(f"    {' ' * len(d['field'])}  got    ={got}")

    # Save JSON report
    report = {
        "total_photos": total_photos,
        "total_ok": total_ok,
        "total_diff": total_diff,
        "overall_accuracy": overall_acc,
        "field_accuracy": {
            field: {
                "errors": field_errors[field],
                "total": field_total[field],
                "accuracy": (field_total[field] - field_errors[field]) / field_total[field] * 100 if field_total[field] > 0 else 0
            }
            for field in COMPARE_FIELDS
        },
        "folders": folder_results,
    }
    report_path = pipe_dir / "accuracy_report.json"
    with open(report_path, "w", encoding="utf-8") as f:
        json.dump(report, f, ensure_ascii=False, indent=2)
    print(f"\nJSON report saved to: {report_path}")

if __name__ == "__main__":
    # Force UTF-8 output on Windows
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8', errors='replace')
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8', errors='replace')
    main()
