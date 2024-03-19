use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use fitrs::Fits;
use fitrs::HeaderValue::{CharacterString, RealFloatingNumber};
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, USER_AGENT};
use serde::{Deserialize, Serialize};


#[derive(Debug, Deserialize, Clone)]
pub struct GraceDbList {
    #[serde(rename = "numRows")]
    num_rows: u64,
    superevents: Vec<GraceDbListEvent>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GraceDbListEvent {
    superevent_id: String,

    #[serde(with = "gracedb_date")]
    created: DateTime<Utc>,
    far: f64,
    links: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GraceDbEvent {
    superevent_id: String,
    alert_type: String,
    #[serde(with = "gracedb_date")]
    time_created: DateTime<Utc>,
    event: GraceDbEventData,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GraceDbEventData {
    significant: bool,
    #[serde(with = "gracedb_date")]
    time: DateTime<Utc>,
    far: f64,
    instruments: Vec<String>,
    group: String,
    pipeline: String,
    search: String,
    properties: GraceDbEventProperties,
    classification: GraceDbEventClassification,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GraceDbEventProperties {
    #[serde(rename = "HasNS")]
    has_ns: f64,
    #[serde(rename = "HasRemnant")]
    has_remnant: f64,
    #[serde(rename = "HasMassGap")]
    has_mass_gap: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GraceDbEventClassification {
    #[serde(rename = "BBH")]
    bbh: f64,
    #[serde(rename = "BNS")]
    bns: f64,
    #[serde(rename = "NSBH")]
    ns_bh: f64,
    #[serde(rename = "Terrestrial")]
    terrestrial: f64,
}

mod gracedb_date {
    use chrono::{DateTime, NaiveDateTime, Utc};
    use serde::{self, Deserialize, Deserializer, Serializer};

    // Events have one of these formats
    const FORMAT_A: &'static str = "%+"; // matches %Y-%m-%dT%H:%M:%SZ";
    const FORMAT_B: &'static str = "%Y-%m-%d %H:%M:%S %Z";

    pub fn serialize<S>(date: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = format!("{}", date.format(FORMAT_A));
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let dt = NaiveDateTime::parse_from_str(&s, FORMAT_A)
            .or_else(|_| NaiveDateTime::parse_from_str(&s, FORMAT_B))
            .map_err(serde::de::Error::custom)?;

        Ok(DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GWEvent {
    id: String,

    #[serde(with = "gracedb_date")]
    time: DateTime<Utc>,

    location_area: u64,
    distance: u64,
    detectors: Vec<String>,
    ns_ns: f64,
    ns_bh: f64,
    bh_bh: f64,
    terrestrial: f64,
    mass_gap: f64,
}

impl std::fmt::Display for GWEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Event: id={:<10} {:<8} time={} area={} dist={} ns_ns={:.3} ns_bh={:.3} bh_bh={:.3} terr={:.3} mass_gap={:.3}",
            self.id,
            self.detectors.join(","),
            self.time,
            self.location_area,
            self.distance,
            self.ns_ns,
            self.ns_bh,
            self.bh_bh,
            self.terrestrial,
            self.mass_gap
    
        )
    }
}

pub type GWEventVec = Vec<GWEvent>;

fn gracedb_to_gwevent(gracedb_event: GraceDbEvent, fits_data: Option<FitsParams>) -> GWEvent {
    GWEvent {
        id: gracedb_event.superevent_id.clone(),
        time: gracedb_event.event.time,
        location_area: 0, // TODO
        distance: fits_data.map_or(0, |d| d.dist_mean as u64),
        detectors: gracedb_event.event.instruments.clone(),
        ns_ns: gracedb_event.event.classification.bns,
        ns_bh: gracedb_event.event.classification.ns_bh,
        bh_bh: gracedb_event.event.classification.bbh,
        terrestrial: gracedb_event.event.classification.terrestrial,
        mass_gap: 0.0, // TODO
    }
}

fn read_gracedbevent(
    url: &String,
    client: &reqwest::blocking::Client,
) -> Result<GraceDbEvent, Box<dyn std::error::Error>> {
    let res = client
        .get(url)
        .header(USER_AGENT, "gwrust")
        .header(ACCEPT, "application/json")
        .send()?
        .error_for_status()?;

    let json = res.text()?;
    println!("Parsing json: {}…", &json[0..60]);

    let conv = serde_json::from_str(&json)?;
    println!("{:?}", conv);
    Ok(conv)
}

fn download_fits(
    file_path: &String,
    url: &String,
    client: &reqwest::blocking::Client,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let cache_folder = Path::new(CACHE_FOLDER);

    let _ = fs::create_dir(cache_folder); // ignore if cache already exists or otherwise fails

    let file_path = cache_folder.join(file_path);
    let metadata = std::fs::metadata(&file_path);

    if metadata.is_ok() {
        println!("File {:?} exists. Not downloading.", &file_path);
        return Ok(file_path);
    }

    let res = client.get(url).header(USER_AGENT, "gwrust").send()?.error_for_status()?;

    let n_bytes = res.content_length().unwrap_or(0);

    let mut file = std::fs::File::create(&file_path)?;
    let mut content = std::io::Cursor::new(res.bytes()?);

    println!("Writing {n_bytes} to {:?}.", &file_path);
    std::io::copy(&mut content, &mut file)?;
    Ok(file_path)
}


#[derive(Debug, Clone)]
struct FitsParams {
    dist_mean: f64,
    dist_std: f64,
    instruments: Vec<String>,
}

const CACHE_FOLDER: &str = "cache";

fn read_fits(filename: &Path) -> Result<FitsParams, Box<dyn std::error::Error>> {
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

// blocking IO
pub fn read_gracedb(last_n: usize) -> Result<Vec<GWEvent>, Box<dyn std::error::Error>> {

    let client = Client::new();
    let mut result: Vec<GWEvent> = Vec::new();

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

    println!("Parsing json: {}…", &text[0..60]);

    let gw: GraceDbList = serde_json::from_str(&text)?;
    for event in gw.superevents.iter().take(last_n) {
        println!("{event:?}");

        if let Some(files) = event.links.get("files") {
            let res = client
                .get(files)
                .header(USER_AGENT, "gwrust")
                .header(ACCEPT, "application/json")
                .send()?
                .error_for_status()?;

            let text = res.text()?;
            let files_map: HashMap<String, String> = serde_json::from_str(&text)?;

            // We are interested in the files update.json, which containes all
            // the obvious metadata for a _confirmed_ event.
            // For sky analysis, we need to look at the file bayestar.mulitorder.fits,
            // which contains the distance and instruments etc.

            let update_json = format!("{}-update.json", event.superevent_id);

            if let Some(url) = files_map.get(&update_json) {
                let eventdata = read_gracedbevent(url, &client)?;
                
                let mut fits_data = None;
                if let Some(url) = files_map.get("bayestar.multiorder.fits") {
                    let gen_name = format!("{}-{}", event.superevent_id, "bayestar.multiorder.fits");
                    let file_path = download_fits(&gen_name, url, &client)?;

                    fits_data = read_fits(&file_path).ok();
                } else {
                    println!("No fits file bayestar.multiorder.fits found. Skipping.")
                }
                
                let gwevent = gracedb_to_gwevent(eventdata, fits_data);
                result.push(gwevent);

            } else {
                println!("Warning: No file {} found for event {}", update_json, event.superevent_id);
            }

        }
    }

    Ok(result)
}
