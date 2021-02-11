use std::boxed::Box;
use std::cell::RefCell;
use std::error::Error;
use std::io::{self, Write};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use curl::easy::{Easy, HttpVersion, List, ReadError};
use curl_sys::{curl_easy_setopt, CURLoption};
use structopt::StructOpt;

const CURLOPT_HTTP09_ALLOWED: CURLoption = 285;

#[derive(Debug, StructOpt)]
#[structopt(name = "ntripping", about = "NTRIP command line client.")]
struct Opt {
    #[structopt(long, default_value = "na.skylark.swiftnav.com:2101/CRS")]
    url: String,

    #[structopt(long, default_value = "37.77101999622968")]
    lat: String,

    #[structopt(long, default_value = "-122.40315159140708")]
    lon: String,

    #[structopt(long, default_value = "-5.549358852471994")]
    height: String,

    #[structopt(short, long)]
    verbose: bool,
}

type Result<T> = std::result::Result<T, Box<dyn Error>>;

thread_local! {
    static CURL: RefCell<Easy> = RefCell::new(Easy::new());
    static LAST: RefCell<SystemTime> = RefCell::new(UNIX_EPOCH);
}

fn checksum(buf: &[u8]) -> u8 {
    let mut sum: u8 = 0;
    for c in &buf[1..] {
        sum ^= c;
    }
    sum
}

fn main() -> Result<()> {
    let opt = Opt::from_args();

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

    CURL.with(|curl| -> Result<()> {
        let mut curl = curl.borrow_mut();

        let mut headers = List::new();
        headers.append("Transfer-Encoding:")?;
        headers.append("Ntrip-Version: Ntrip/2.0")?;

        curl.http_headers(headers)?;
        curl.useragent("NTRIP ntrip-client/1.0")?;
        curl.url(&opt.url)?;
        curl.progress(true)?;
        curl.put(true)?;
        curl.custom_request("GET")?;
        curl.http_version(HttpVersion::Any)?;

        unsafe { curl_easy_setopt(curl.raw(), CURLOPT_HTTP09_ALLOWED, 1) };

        if opt.verbose {
            curl.verbose(true)?;
        }

        curl.write_function(|buf| Ok(io::stdout().write_all(buf).map_or(0, |_| buf.len())))?;

        curl.progress_function(|_dltot, _dlnow, _ultot, _ulnow| {
            let now = SystemTime::now();
            let elapsed = LAST.with(|last| {
                let dur = now.duration_since(*last.borrow());
                dur.unwrap_or_else(|_| Duration::from_secs(0)).as_secs()
            });
            if elapsed > 10 {
                CURL.with(|curl| curl.borrow().unpause_read().unwrap());
            }
            true
        })?;

        curl.read_function(move |mut buf: &mut [u8]| {
            let now = SystemTime::now();
            let elapsed = LAST.with(|last| {
                let dur = now.duration_since(*last.borrow());
                dur.unwrap_or_else(|_| Duration::from_secs(0)).as_secs()
            });
            if elapsed > 10 {
                LAST.with(|last| *last.borrow_mut() = now);
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
        })?;

        Ok(())
    })?;

    CURL.with(|curl| -> Result<()> { Ok(curl.borrow().perform()?) })?;

    Ok(())
}
