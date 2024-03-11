use std::fs::File;
use std::{collections::HashMap, error::Error};

use chrono::NaiveDate;
use csv::ReaderBuilder;
use reqwest::header::ACCEPT;
use reqwest::Client;
use serde::{Deserialize, Deserializer};

// (0)GW event, Detection time, Location area, Luminosity distance,
// (4)Detector, False Alarm Rate, False Alarm chance in O4,
// (7) NS / NS, NS / BH, BH / BH, Mass gap, Terrestrial,
// (12) Notes, Ref

#[derive(Debug, Deserialize, Clone)]
pub struct GraceDbList {
    numRows: u64,
    superevents: Vec<GraceDbListEvent>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GraceDbListEvent {
    superevent_id: String,
    created: String,
    far: f64,
    links: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GraceDbEvent {
    superevent_id: String,
    alert_type: String,
    time_created: String,
    event: GraceDbEventData,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GraceDbEventData {
    significant: bool,
    time: String,
    far: f64,
    instruments: Vec<String>,
    group: String,
    pipeline: String,
    search: String,
    properties: GraceDbEventProperties,
    classification: GraceDbEventClassification,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GraceDbEventProperties {
    HasNS: f64,
    HasRemnant: f64,
    HasMassGap: f64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GraceDbEventClassification {
    BBH: f64,
    BNS: f64,
    NSBH: f64,
    Terrestrial: f64,
}

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
    NaiveDate::parse_from_str(date_str, "%Y-%m-%d").map_err(serde::de::Error::custom)
}

fn deserialize_detectors<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let buf = String::deserialize(deserializer)?;
    let detectors = buf.split(",").map(|i| i.to_string()).collect::<Vec<String>>();
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

fn read_gracedbevent(
    url: &String,
    client: &reqwest::blocking::Client,
) -> Result<GraceDbEvent, Box<dyn std::error::Error>> {
    use reqwest::header::{CONTENT_TYPE, USER_AGENT};

    let res = client
        .get(url)
        .header(USER_AGENT, "gwrust")
        .header(ACCEPT, "application/json")
        .send()?
        .error_for_status()?;

    let json = res.text()?;

    let conv = serde_json::from_str(&json)?;
    Ok(conv)
}

#[derive(Debug, Clone)]
struct FitsParams {
    dist_mean: f64,
    dist_std: f64,
    instruments: Vec<String>,
}

fn read_fits(filename: &str) -> Result<FitsParams, Box<dyn std::error::Error>> {
    use fitrs::Fits;
    use fitrs::HeaderValue::{CharacterString, RealFloatingNumber};

    let fits = Fits::open(filename).expect("Failed to open");

    let mut dist_mean = 0.0;
    let mut dist_std = 0.0;
    let mut instruments: Vec<String> = Vec::new();

    // Iterate over HDUs
    for hdu in fits.iter() {
        for (header, value) in hdu.iter() {
            if header == "DISTMEAN" {
                if let Some(RealFloatingNumber(v)) = value {
                    dist_mean = *v;
                }
            }
            if header == "DISTSTD" {
                if let Some(RealFloatingNumber(v)) = value {
                    dist_std = *v;
                }
            }
            if header == "INSTRUME" {
                if let Some(CharacterString(v)) = value {
                    instruments = v.split(",").map(|x| x.to_string()).collect();
                }
            }
            println!("{:?} {:?}", header, value);
        }
    }

    Ok(FitsParams { dist_mean, dist_std, instruments })
}

pub fn read_gracedb() -> Result<(), Box<dyn std::error::Error>> {
    use reqwest::blocking::Client;
    use reqwest::header::{CONTENT_TYPE, USER_AGENT};

    // read fits file
    println!("{:?}", read_fits("bayestar.fits"));

    let client = Client::new();

    let url = "https://gracedb.ligo.org/apiweb/superevents/";
    let res = client
        .get(url)
        .header(USER_AGENT, "gwrust")
        .header(ACCEPT, "application/json")
        .query(&[("query", "category: Production label: SIGNIF_LOCKED")])
        .send()?
        .error_for_status()?;
    //let content = res;
    let text = res.text()?;
    println!("Out: {text:?}");

    println!("");

    let gw: GraceDbList = serde_json::from_str(&text)?;
    for event in gw.superevents {
        println!("{event:?}");

        if let Some(files) = event.links.get("files") {
            let res = client
                .get(files)
                .header(USER_AGENT, "gwrust")
                .header(ACCEPT, "application/json")
                .send()?
                .error_for_status()?;
            //let content = res;
            let text = res.text()?;
            let files_map: HashMap<String, String> = serde_json::from_str(&text)?;
            for f in files_map.keys() {
                if f.contains("update.json") {
                    let name = files_map.get(f);
                    if let Some(name) = name {
                        let eventdata = read_gracedbevent(name, &client)?;
                        println!("Eventdata: {eventdata:?}");
                    }
                    println!("{name:?}");
                }
            }
            //println!("Out: {files_map:?}");
        }
    }

    Ok(())
}
