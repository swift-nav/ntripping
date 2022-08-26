use std::boxed::Box;
use std::cell::RefCell;
use std::error::Error;
use std::io::{self, Write};
use std::ops::Add;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use clap::Parser;
use curl::easy::{Easy, HttpVersion, List, ReadError};

#[derive(Debug, Parser)]
#[clap(name = "ntripping", about = "NTRIP command line client.", version = env!("VERGEN_SEMVER_LIGHTWEIGHT"))]
struct Cli {
    /// URL of the NTRIP caster
    #[clap(long, default_value = "na.skylark.swiftnav.com:2101/CRS")]
    url: String,

    /// Receiver latitude to report, in degrees
    #[clap(long, default_value = "37.77101999622968", allow_hyphen_values = true)]
    lat: String,

    /// Receiver longitude to report, in degrees
    #[clap(
        long,
        default_value = "-122.40315159140708",
        allow_hyphen_values = true
    )]
    lon: String,

    /// Receiver height to report, in meters
    #[clap(long, default_value = "-5.549358852471994", allow_hyphen_values = true)]
    height: String,

    /// Client ID
    #[clap(long, default_value = "00000000-0000-0000-0000-000000000000")]
    client: String,

    #[clap(short, long)]
    verbose: bool,

    /// Receiver time to report, as a Unix time
    #[clap(long)]
    epoch: Option<u32>,

    /// Username credentials
    #[clap(long)]
    username: Option<String>,

    /// Password credentials
    #[clap(long)]
    password: Option<String>,
}

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn checksum(buf: &[u8]) -> u8 {
    let mut sum: u8 = 0;
    for c in &buf[1..] {
        sum ^= c;
    }
    sum
}

fn main() -> Result<()> {
    let opt = Cli::parse();

    let latf: f64 = opt.lat.parse::<f64>()?;
    let lonf: f64 = opt.lon.parse::<f64>()?;
    let heightf: f64 = opt.height.parse::<f64>()?;

    let latn = ((latf * 1e8).round() / 1e8).abs();
    let lonn = ((lonf * 1e8).round() / 1e8).abs();

    let lat_deg: u16 = latn as u16;
    let lon_deg: u16 = lonn as u16;

    let lat_min: f64 = (latn - (lat_deg as f64)) * 60.0;
    let lon_min: f64 = (lonn - (lon_deg as f64)) * 60.0;

    let lat_dir = if latf < 0.0 { 'S' } else { 'N' };
    let lon_dir = if lonf < 0.0 { 'W' } else { 'E' };

    let mut curl = Easy::new();
    let last_time_ref = Arc::new(RefCell::new(UNIX_EPOCH));

    let mut headers = List::new();
    let mut client_header = "X-SwiftNav-Client-Id: ".to_string();
    client_header.push_str(&opt.client);

    headers.append("Transfer-Encoding:")?;
    headers.append("Ntrip-Version: Ntrip/2.0")?;
    headers.append(&client_header)?;

    curl.http_headers(headers)?;
    curl.useragent("NTRIP ntrip-client/1.0")?;
    curl.url(&opt.url)?;
    curl.progress(true)?;
    curl.put(true)?;
    curl.custom_request("GET")?;
    curl.http_version(HttpVersion::Any)?;
    curl.http_09_allowed(true)?;

    if opt.verbose {
        curl.verbose(true)?;
    }

    if let Some(username) = &opt.username {
        curl.username(username)?;
    }

    if let Some(password) = &opt.password {
        curl.password(password)?;
    }

    curl.write_function(|buf| Ok(io::stdout().write_all(buf).map_or(0, |_| buf.len())))?;

    {
        let mut curl= curl.transfer();

        progress_transfer.progress_function({
            let last_time_ref = last_time_ref.clone();
            move |_dltot, _dlnow, _ultot, _ulnow| {
                let now = SystemTime::now();
                let elapsed = {
                    let dur = now.duration_since(*last_time_ref.borrow());
                    dur.unwrap_or_else(|_| Duration::from_secs(0)).as_secs()
                };
                if elapsed > 10 {
                    curl_ref.unpause_read().unwrap();
                }
                true
            }
        })?;
    }

    let read_transfer = curl.transfer();

    read_transfer.read_function({
        let last_time_ref = last_time_ref.clone();
        move |mut buf: &mut [u8]| {
            let now = if let Some(epoch) = opt.epoch {
                SystemTime::UNIX_EPOCH.add(Duration::from_secs(epoch.into()))
            } else {
                SystemTime::now()
            };
            let elapsed = {
                let dur = now.duration_since(*last_time_ref.borrow());
                dur.unwrap_or_else(|_| Duration::from_secs(0)).as_secs()
            };
            if elapsed > 10 {
                *last_time_ref.borrow_mut() = now;
                let datetime: DateTime<Utc> = now.into();
                let time = datetime.format("%H%M%S.00");
                let gpgga = format!(
                    "$GPGGA,{},{:02}{:010.7},{},{:03}{:010.7},{},4,12,1.3,{:.2},M,0.0,M,1.7,0078",
                    time, lat_deg, lat_min, lat_dir, lon_deg, lon_min, lon_dir, heightf
                );
                let checksum = checksum(gpgga.as_bytes());
                let gpgga = format!("{}*{:X}\r\n", gpgga, checksum);
                buf.write_all(gpgga.as_bytes()).unwrap();
                Ok(buf.len())
            } else {
                Err(ReadError::Pause)
            }
        }
    })?;

    curl.perform()?;

    Ok(())
}
