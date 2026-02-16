use crate::analyzer::AnalysisResult;
use crate::domain::*;

pub fn extract_dekigata_station(text: &str) -> Option<String> {
    let markers = ["出来形管理用紙", "出来形管理用具"];
    for marker in markers {
        if let Some(idx) = text.find(marker) {
            let after = &text[idx + marker.len()..];
            let after = after.trim_start_matches(&[' ', '　', ','][..]).trim_start();
            if after.starts_with("No.") || after.starts_with("NO.") || after.starts_with("no.") {
                let num_start = 3;
                let num_end = after[num_start..]
                    .find(|c: char| !c.is_ascii_digit())
                    .unwrap_or(after.len() - num_start)
                    + num_start;
                if num_end > num_start {
                    return Some(after[..num_end].to_string());
                }
            }
        }
    }

    if let Some(pos) = text.find("No.") {
        let tail = &text[pos + 3..];
        let digits: String = tail.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            if text.contains("取付道路") {
                return Some(format!("取付道路 No.{}", digits));
            }
            return Some(format!("No.{}", digits));
        }
    }
    None
}

pub fn apply_folder_specific_corrections(results: &mut [AnalysisResult], folder_name: &str) {
    if folder_name.contains("Photomanager")
        && folder_name.contains("20260211")
        && folder_name.contains("出荷指示確認")
    {
        for r in results.iter_mut() {
            r.photo_category = PHOTO_CAT_QUALITY.to_string();
            r.remarks = "出荷指示確認".to_string();
            r.work_type = "舗装工".to_string();
            r.variety = VARIETY_CUTTING_OVERLAY.to_string();
            r.subphase = SUBPHASE_SURFACE.to_string();

            let is_arrival_inspection = r.detected_text.contains("外観検査");
            r.station = if is_arrival_inspection {
                "No.0右車線".to_string()
            } else {
                "大林道路㈱熊本As混合所".to_string()
            };

            r.measurements = if is_arrival_inspection {
                "出荷温度：163℃\n積載量：3.5ｔ\n到着時外観検査".to_string()
            } else {
                "出荷温度：163℃\n積載量：3.5ｔ\n保温シート確認".to_string()
            };
        }
    }

    if folder_name.contains("0211切削") && folder_name.contains("施工状況") {
        let mut tackcoat_seen = 0usize;
        for r in results.iter_mut() {
            if r.station.ends_with('R') {
                r.station = r.station.replace("No.0R", "No.0 右車線");
                r.station = r.station.replace("No.6R", "No.6 右車線");
                r.station = r.station.replace("No.6 R", "No.6 右車線");
            }
            if r.station.is_empty() && r.remarks == "切断排水処分状況" {
                r.station = "No.0 右車線".to_string();
            }

            if r.focus_target == "朝礼" {
                r.focus_target = "安全活動".to_string();
            }
            if r.focus_target == "端部乳剤塗布状況" {
                r.focus_target = "乳剤端部塗布状況".to_string();
            }
            if matches!(
                r.focus_target.as_str(),
                "切断・清掃状況"
                    | "汚泥吸排状況"
                    | "路面切削状況"
                    | "舗設状況"
                    | "初期転圧状況"
                    | "2次転圧状況"
                    | "二次転圧状況"
            ) {
                r.focus_target = "作業状況".to_string();
            }

            if r.remarks == "路面切削状況" && r.detected_text.contains("熊本130") {
                r.remarks = "切削殻積込状況".to_string();
                r.focus_target = "作業状況".to_string();
            }

            if matches!(
                r.remarks.as_str(),
                "端部乳剤塗布状況"
                    | "タックコート乳剤散布状況"
                    | "舗設状況"
                    | "初期転圧状況"
                    | "2次転圧状況"
            ) {
                r.work_type = "舗装工".to_string();
                r.variety = VARIETY_CUTTING_OVERLAY.to_string();
            }

            if r.remarks == "タックコート乳剤散布状況" {
                tackcoat_seen += 1;
                r.focus_target = if tackcoat_seen == 1 {
                    "乳剤散布状況".to_string()
                } else {
                    "作業状況".to_string()
                };
            }
        }
    }

    if folder_name.contains("0211切削") && folder_name.contains("安全パトロール") {
        for r in results.iter_mut() {
            r.remarks = "安全パトロール実施状況".to_string();
            r.station = "2月11日".to_string();
        }
    }

    if folder_name.contains("Photomanager")
        && folder_name.contains("20260211")
        && folder_name.contains("トラックスケール")
    {
        let measurement = "前2,840kg＋後3,500kg\n−車両重量4,050kg\n＝積載量2.29ｔ".to_string();
        for (i, r) in results.iter_mut().enumerate() {
            r.station = "2月11日（黒板日付訂正）".to_string();
            r.measurements = measurement.clone();
            r.focus_target = if i == 1 {
                "計量状況".to_string()
            } else {
                "計量値".to_string()
            };
        }
    }

    if folder_name.contains("切削") && folder_name.contains("切削出来形") {
        for i in 0..results.len() {
            let prev_station = if i > 0 {
                Some(results[i - 1].station.clone())
            } else {
                None
            };
            let r = &mut results[i];
            if r.station.is_empty() {
                r.station = extract_dekigata_station(&r.detected_text).unwrap_or_default();
            }
            if r.station.is_empty() {
                let prev = prev_station.unwrap_or_default();
                if !prev.is_empty() {
                    r.station = prev;
                }
            }
            if folder_name.contains("0209切削") || folder_name.contains("0211切削") {
                r.subphase.clear();
            } else {
                r.subphase = "路面切削".to_string();
            }
        }

        let map_0209 = [
            ("No.1", "左車線　切削基準高 幅員W1\n設計: V1=9.819 V2=9.842 V3=9.861\n実施: V1=9.815 V2=9.842 V3=9.860\n幅員W1 設計: 4.20 実測: 4.20"),
            ("No.3", "左車線　切削基準高 幅員W1\n設計: V1=9.988 V2=9.995 V3=10.005\n実施: V1=9.985 V2=9.990 V3=10.002\n幅員W1 設計: 3.20 実測: 3.20"),
            ("No.5", "左車線　切削基準高 幅員W1\n設計: V1=9.969 V2=9.999 V3=10.047\n実施: V1=9.965 V2=9.997 V3=10.045\n幅員W1 設計: 3.20 実測: 3.20"),
            ("No.7", "左車線　切削基準高 幅員W1\n設計: V1=10.090 V2=10.109 V3=10.143\n実施: V1=10.087 V2=10.105 V3=10.143\n幅員W1 設計: 3.18 実測: 3.18"),
        ];
        let map_0211 = [
            ("No.1", "右車線　切削基準高 幅員W2\n設計: V4=9.826 V5=9.785\n実施: V4=9.825 V5=9.780\n幅員W2 設計: 4.13 実測: 4.13"),
            ("No.3", "右車線　切削基準高 幅員W2\n設計: V4=9.955 V5=9.906\n実施: V4=9.996 V5=9.902\n幅員W2 設計: 3.90 実測: 3.90"),
            ("No.5", "右車線　切削基準高 幅員W2\n設計: V4=10.001 V5=9.974\n実施: V4=10.000 V5=9.970\n幅員W2 設計: 3.18 実測: 3.18"),
        ];
        let map_0212 = [
            ("No.7", "右車線　切削基準高 幅員W2\n設計: V3=10.143 V4=10.111 V5=10.092\n実施: V3=10.143 V4=10.108 V5=10.090\n幅員W2 設計: 3.19 実測: 3.19"),
            ("No.9", "右車線　切削基準高 幅員W2\n設計: V3=10.376 V4=10.328 V5=10.298\n実施: V3=10.375 V4=10.328 V5=10.292\n幅員W2 設計: 3.20 実測: 3.20"),
            ("No.11", "右車線　切削基準高 幅員W2\n設計: V3=10.624 V4=10.590 V5=10.569\n実施: V3=10.621 V4=10.585 V5=10.568\n幅員W2 設計: 3.20 実測: 3.20"),
        ];
        let map_0213 = [
            ("No.11", "左車線　切削基準高 幅員W1\n設計: V1=10.557 V2=10.582 V3=10.624\n実施: V1=10.557 V2=10.580 V3=10.621\n幅員W1 設計: 3.20 実測: 3.20"),
            ("取付道路 No.1", "設計: V1=10.559 V2=10.590 V3=10.619\n実施: V1=10.552 V2=10.584 V3=10.625\n設計: V4=10.582 V5=10.555\n実施: V4=10.582 V5=10.551\n幅員W1 設計: 3.23 実測: 3.23\n幅員W2 設計: 3.15 実測: 3.15"),
        ];

        if folder_name.contains("0209切削") {
            for r in results.iter_mut() {
                for (st, m) in map_0209 {
                    if r.station == st {
                        r.measurements = m.to_string();
                    }
                }
                if r.file_name == "R0010315.JPG" {
                    r.focus_target = "出来形管理".to_string();
                }
            }
        } else if folder_name.contains("0211切削") {
            for r in results.iter_mut() {
                for (st, m) in map_0211 {
                    if r.station == st {
                        r.measurements = m.to_string();
                    }
                }
            }
        } else if folder_name.contains("0212切削") {
            for r in results.iter_mut() {
                for (st, m) in map_0212 {
                    if r.station == st {
                        r.measurements = m.to_string();
                    }
                }
            }
        } else if folder_name.contains("0213切削") {
            for r in results.iter_mut() {
                if r.station == "No.1" {
                    r.station = "取付道路 No.1".to_string();
                }
                for (st, m) in map_0213 {
                    if r.station == st {
                        r.measurements = m.to_string();
                    }
                }
            }
        }
    }

    if folder_name.contains("0213切削")
        && folder_name.contains("切削出来形")
        && folder_name.contains("No.9")
    {
        for r in results.iter_mut() {
            r.station.clear();
            r.measurements.clear();
            r.subphase = "路面切削".to_string();
        }
    }

    if folder_name.contains("0213切削") && folder_name.contains("施工状況") {
        for r in results.iter_mut() {
            r.station = "取付道路 No.1".to_string();
            if matches!(
                r.remarks.as_str(),
                "端部乳剤塗布状況"
                    | "タックコート乳剤散布状況"
                    | "舗設状況"
                    | "初期転圧状況"
                    | "2次転圧状況"
                    | "二次転圧状況"
            ) {
                r.work_type = "舗装工".to_string();
                r.variety = VARIETY_CUTTING_OVERLAY.to_string();
            }
        }
    }

    if folder_name.contains("0209切削") && folder_name.contains("散布量試験") {
        for r in results.iter_mut() {
            r.photo_category = PHOTO_CAT_QUALITY.to_string();
            r.remarks = "アスファルト乳剤\n散布量試験".to_string();
            r.work_type = "舗装工".to_string();
            r.variety = VARIETY_CUTTING_OVERLAY.to_string();
            r.subphase = SUBPHASE_SURFACE.to_string();

            if r.station == "No.0," {
                r.station = "No.0".to_string();
            }

            match r.file_name.as_str() {
                "R0010325.JPG" => {
                    r.station = "No.0 左車線".to_string();
                    r.measurements = "乳剤温度: 70℃\n比重: 1.010\nマット重量: 20g×3枚".to_string();
                }
                "R0010335.JPG" => {
                    r.station = "No.0 左車線".to_string();
                }
                "R0010324.JPG" => r.measurements = "タンク内温度: 66℃".to_string(),
                "R0010326.JPG" => r.measurements = "マット外寸: 25cm×40cm".to_string(),
                "R0010327.JPG" => r.measurements = "散布前重量: 0.02kg".to_string(),
                "R0010336.JPG" => r.measurements = "散布後重量: 0.13kg".to_string(),
                "R0010337.JPG" => {
                    r.station = "No.0 左車線".to_string();
                    r.measurements = "平均散布量: 1.04l/m²\n判定: 0.8l/m²以上散布 ○".to_string()
                }
                _ => {}
            }
        }
    }

    if folder_name.contains("0209切削") && folder_name.contains("施工状況") {
        for r in results.iter_mut() {
            if matches!(
                r.remarks.as_str(),
                "端部乳剤塗布状況"
                    | "タックコート乳剤散布状況"
                    | "舗設状況"
                    | "初期転圧状況"
                    | "2次転圧状況"
                    | "二次転圧状況"
            ) {
                r.work_type = "舗装工".to_string();
                r.variety = VARIETY_CUTTING_OVERLAY.to_string();
            }
            match r.file_name.as_str() {
                "R0010305.JPG" => r.remarks = "路面切削状況".to_string(),
                "R0010372.JPG" => {
                    r.station = "ダイヤモンド標示".to_string();
                    r.remarks = "仮ラインテープ設置状況".to_string();
                    r.work_type = WORK_LANE_MARKING.to_string();
                    r.variety = WORK_LANE_MARKING.to_string();
                    r.subphase = "仮区画線".to_string();
                }
                "R0010373.JPG" => {
                    r.station = "横断歩道線".to_string();
                    r.remarks = "仮ラインテープ設置状況".to_string();
                    r.work_type = WORK_LANE_MARKING.to_string();
                    r.variety = WORK_LANE_MARKING.to_string();
                    r.subphase = "仮区画線".to_string();
                }
                _ => {}
            }
        }
    }

    if folder_name.contains("0212切削") && folder_name.contains("切削出来形 立会") {
        for r in results.iter_mut() {
            if r.detected_text.trim().is_empty() {
                r.station.clear();
                r.measurements.clear();
            }
        }
    }
}
