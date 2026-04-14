// golden_diff.rs — Rust side of the Rust ↔ Go golden-diff harness (issue #165 Stream E).
//
// Reads fixtures from tests/golden_diff/fixtures/<name>/{input,expected}.json
// relative to the workspace root (CARGO_MANIFEST_DIR/../..).
// Runs the photo-ai-common implementation, serialises output to the same JSON
// schema as the Go side, and asserts per-row equality.
//
// Run with:
//   cargo test -p photo-ai-common --test golden_diff

use photo_ai_common::domain::temperature::TemperatureKind;
use photo_ai_common::domain::station::Station;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn fixture_dir(name: &str) -> PathBuf {
    // CARGO_MANIFEST_DIR is common/ — workspace root is one level up.
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .expect("workspace root")
        .join("tests")
        .join("golden_diff")
        .join("fixtures")
        .join(name)
}

fn read_json_file<T: for<'de> Deserialize<'de>>(path: &Path) -> T {
    let data = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", path.display(), e));
    serde_json::from_str(&data)
        .unwrap_or_else(|e| panic!("cannot parse {}: {}", path.display(), e))
}

// ─── temperature_normalization ────────────────────────────────────────────────

#[derive(Deserialize)]
struct TempInput {
    id: String,
    text: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct TempRow {
    id: String,
    kind: String,
    label: String,
    short_label: String,
    found: bool,
}

fn kind_name(k: TemperatureKind) -> &'static str {
    match k {
        TemperatureKind::Arrival           => "TemperatureArrival",
        TemperatureKind::Spreading         => "TemperatureSpreading",
        TemperatureKind::InitialCompaction => "TemperatureInitialCompaction",
        TemperatureKind::Opening           => "TemperatureOpening",
        TemperatureKind::OutsideAir        => "TemperatureOutsideAir",
    }
}

#[test]
fn temperature_normalization_matches_golden() {
    let dir = fixture_dir("temperature_normalization");
    let inputs: Vec<TempInput>   = read_json_file(&dir.join("input.json"));
    let expected: Vec<TempRow>   = read_json_file(&dir.join("expected.json"));

    assert_eq!(
        inputs.len(), expected.len(),
        "fixture length mismatch (input vs expected)"
    );

    for (input, exp) in inputs.iter().zip(expected.iter()) {
        assert_eq!(input.id, exp.id, "row ID order mismatch");

        let actual = match TemperatureKind::from_text(&input.text) {
            Some(k) => TempRow {
                id:          input.id.clone(),
                kind:        kind_name(k).to_string(),
                label:       k.label().to_string(),
                short_label: k.short_label().to_string(),
                found:       true,
            },
            None => TempRow {
                id:          input.id.clone(),
                kind:        String::new(),
                label:       String::new(),
                short_label: String::new(),
                found:       false,
            },
        };

        // Compare via JSON to match Go's field ordering / serialisation.
        let act_json = serde_json::to_string(&actual).unwrap();
        let exp_json = serde_json::to_string(exp).unwrap();
        assert_eq!(
            act_json, exp_json,
            "[{}] temperature mismatch\n  got : {}\n  want: {}",
            input.id, act_json, exp_json
        );
    }
}

// ─── station_normalization ────────────────────────────────────────────────────

#[derive(Deserialize)]
struct StationInput {
    id: String,
    text: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct StationRow {
    id: String,
    kind: String,
    display: String,
    is_post: bool,
    is_date: bool,
    has_dump_number: bool,
}

fn station_kind_name(s: &Station) -> &'static str {
    match s {
        Station::Empty    => "Empty",
        Station::Post(_)  => "Post",
        Station::Date(_)  => "Date",
        Station::Other(_) => "Other",
    }
}

#[test]
fn station_normalization_matches_golden() {
    let dir = fixture_dir("station_normalization");
    let inputs: Vec<StationInput>   = read_json_file(&dir.join("input.json"));
    let expected: Vec<StationRow>   = read_json_file(&dir.join("expected.json"));

    assert_eq!(
        inputs.len(), expected.len(),
        "fixture length mismatch (input vs expected)"
    );

    for (input, exp) in inputs.iter().zip(expected.iter()) {
        assert_eq!(input.id, exp.id, "row ID order mismatch");

        let s = Station::parse(&input.text);
        let actual = StationRow {
            id:              input.id.clone(),
            kind:            station_kind_name(&s).to_string(),
            display:         s.to_display(),
            is_post:         s.is_post(),
            is_date:         s.is_date(),
            has_dump_number: s.has_dump_number(),
        };

        let act_json = serde_json::to_string(&actual).unwrap();
        let exp_json = serde_json::to_string(exp).unwrap();
        assert_eq!(
            act_json, exp_json,
            "[{}] station mismatch\n  got : {}\n  want: {}",
            input.id, act_json, exp_json
        );
    }
}
