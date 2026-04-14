/// EXIF extraction using kamadak-exif crate.
use photo_ai_common::normalize_exif_datetime;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use crate::types::{EngineResponse, to_json_string};

#[derive(Deserialize, Debug)]
pub struct ExifConfig {
    pub image_path: String,
    pub raw: Option<bool>,
}

#[derive(Serialize, Debug)]
pub struct GpsCoords {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: Option<f64>,
}

#[derive(Serialize, Debug)]
pub struct ExifResult {
    pub fields: HashMap<String, String>,
    pub gps: Option<GpsCoords>,
    pub datetime: Option<String>,
}

pub fn extract_exif(config: ExifConfig) -> String {
    match extract_exif_inner(&config) {
        Ok(result) => {
            let resp: EngineResponse<ExifResult> = EngineResponse::success(result);
            to_json_string(&resp)
        }
        Err(e) => {
            let resp: EngineResponse<ExifResult> = EngineResponse::failure(e);
            to_json_string(&resp)
        }
    }
}

fn extract_exif_inner(config: &ExifConfig) -> Result<ExifResult, String> {
    let file = File::open(&config.image_path)
        .map_err(|e| format!("failed to open file: {}", e))?;
    let mut bufreader = BufReader::new(file);
    let exif_reader = exif::Reader::new();
    let exif_data = exif_reader
        .read_from_container(&mut bufreader)
        .map_err(|e| format!("failed to read EXIF: {}", e))?;

    let raw = config.raw.unwrap_or(false);
    let mut fields = HashMap::new();
    let mut datetime: Option<String> = None;
    let mut gps = GpsState::default();

    for field in exif_data.fields() {
        let tag_name = format!("{}", field.tag);
        let value_str = field.display_value().to_string();

        // Extract datetime
        // kamadak-exif は仕様どおり `YYYY:MM:DD HH:MM:SS` を返すため、
        // 下流の chrono パーサ等が扱えるよう ISO風に正規化して格納する。
        if field.tag == exif::Tag::DateTimeOriginal || field.tag == exif::Tag::DateTime {
            if datetime.is_none() {
                datetime = Some(normalize_exif_datetime(&value_str));
            }
        }

        // Extract GPS
        match field.tag {
            exif::Tag::GPSLatitude => {
                gps.latitude = parse_gps_dms(&field.value);
            }
            exif::Tag::GPSLatitudeRef => {
                gps.lat_ref = Some(value_str.clone());
            }
            exif::Tag::GPSLongitude => {
                gps.longitude = parse_gps_dms(&field.value);
            }
            exif::Tag::GPSLongitudeRef => {
                gps.lon_ref = Some(value_str.clone());
            }
            exif::Tag::GPSAltitude => {
                gps.altitude = parse_rational_single(&field.value);
            }
            exif::Tag::GPSAltitudeRef => {
                gps.alt_ref = parse_altitude_ref(&field.value);
            }
            _ => {}
        }

        if raw {
            fields.insert(tag_name, value_str);
        }
    }

    // If not raw mode, only include date and gps-related fields
    if !raw {
        if let Some(ref dt) = datetime {
            fields.insert("DateTime".to_string(), dt.clone());
        }
    }

    let gps_coords = gps.to_coords();

    Ok(ExifResult {
        fields,
        gps: gps_coords,
        datetime,
    })
}

#[derive(Default)]
struct GpsState {
    latitude: Option<f64>,
    lat_ref: Option<String>,
    longitude: Option<f64>,
    lon_ref: Option<String>,
    altitude: Option<f64>,
    alt_ref: Option<i8>,
}

impl GpsState {
    fn to_coords(&self) -> Option<GpsCoords> {
        let lat = self.latitude?;
        let lon = self.longitude?;

        let lat = if self.lat_ref.as_deref() == Some("S") { -lat } else { lat };
        let lon = if self.lon_ref.as_deref() == Some("W") { -lon } else { lon };

        let altitude = self.altitude.map(|alt| {
            if self.alt_ref == Some(1) { -alt } else { alt }
        });

        Some(GpsCoords {
            latitude: lat,
            longitude: lon,
            altitude,
        })
    }
}

/// Parse GPS DMS (degrees, minutes, seconds) from EXIF rational values
fn parse_gps_dms(value: &exif::Value) -> Option<f64> {
    match value {
        exif::Value::Rational(rats) if rats.len() >= 3 => {
            let deg = rats[0].to_f64();
            let min = rats[1].to_f64();
            let sec = rats[2].to_f64();
            Some(deg + min / 60.0 + sec / 3600.0)
        }
        _ => None,
    }
}

/// Parse a single rational value (e.g., altitude)
fn parse_rational_single(value: &exif::Value) -> Option<f64> {
    match value {
        exif::Value::Rational(rats) if !rats.is_empty() => Some(rats[0].to_f64()),
        _ => None,
    }
}

/// Parse altitude ref (0 = above sea level, 1 = below)
fn parse_altitude_ref(value: &exif::Value) -> Option<i8> {
    match value {
        exif::Value::Byte(bytes) if !bytes.is_empty() => Some(bytes[0] as i8),
        _ => None,
    }
}
