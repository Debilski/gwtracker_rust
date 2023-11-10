use std::error::Error;
use std::fs::File;

use chrono::NaiveDate;
use csv::ReaderBuilder;
use serde::{Deserialize, Deserializer};

// (0)GW event, Detection time, Location area, Luminosity distance,
// (4)Detector, False Alarm Rate, False Alarm chance in O4,
// (7) NS / NS, NS / BH, BH / BH, Mass gap, Terrestrial,
// (12) Notes, Ref

#[derive(Debug, Deserialize, Clone)]
pub struct GWData {
    id: String,

    #[serde(deserialize_with = "deserialize_date")]
    detection_time: NaiveDate,
    location_area: u64,
    #[serde(deserialize_with = "deserialize_pm")]
    distance: u64,
    #[serde(deserialize_with = "deserialize_detectors")]
    detectors: Vec<String>,
    #[serde(deserialize_with = "deserialize_pmf")]
    NS_NS: f64,
    #[serde(deserialize_with = "deserialize_pmf")]
    NS_BH: f64,
    #[serde(deserialize_with = "deserialize_pmf")]
    BH_BH: f64,
    #[serde(deserialize_with = "deserialize_pmf")]
    mass_gap: f64,
    //#[serde(deserialize_with = "deserialize_pm")]
    //terrestrial: Option<f64>
}

fn deserialize_date<'de, D>(deserializer: D) -> Result<NaiveDate, D::Error>
where
    D: Deserializer<'de>,
{
    let buf = String::deserialize(deserializer)?;
    let date_str = buf.split_whitespace().next().unwrap_or("");
    let r = NaiveDate::parse_from_str(date_str, "%Y-%m-%d").map_err(serde::de::Error::custom);
    r
}

fn deserialize_detectors<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let buf = String::deserialize(deserializer)?;
    let detectors = buf
        .split(",")
        .map(|i| i.to_string())
        .collect::<Vec<String>>();
    Ok(detectors)
}

fn deserialize_pm<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let buf = String::deserialize(deserializer)?;

    let split = buf.split(&['±']).next().unwrap_or("");
    str::parse::<u64>(&split).map_err(serde::de::Error::custom)
}

fn deserialize_pmf<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let buf = String::deserialize(deserializer)?;

    let split = buf.split(&['±']).next().unwrap_or("");
    str::parse::<f64>(&split).map_err(serde::de::Error::custom)
}

pub fn read_tsv(file_path: &str) -> Result<Vec<GWData>, Box<dyn Error>> {
    let file = File::open(file_path)?;
    let mut tsv_reader = ReaderBuilder::new()
        .has_headers(true)
        .delimiter(b'\t')
        .from_reader(file);

    let mut results: Vec<GWData> = Vec::new();

    for result in tsv_reader.deserialize() {
        if let Ok(record) = result {
            results.push(record);
        }
    }
    Ok(results)
}
