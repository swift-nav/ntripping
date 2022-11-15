use std::boxed::Box;
use std::cell::RefCell;
use std::error::Error;
use std::io::{self, Write};
use std::ops::Add;
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
    client_id: String,

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

    /// GGA update period, in seconds. 0 means to never send a GGA
    #[clap(long, default_value = "10")]
    gga_period: u64,

    /// AreaID to be used in generation of CRA message. If this flag is set, ntripping outputs messages of type CRA rather than the default GGA.
    #[clap(long)]
    area_id: Option<u32>,

    /// SolutionID, the identifier of the connection stream to reconnect to in the event of disconnections
    #[clap(long)]
    solution_id: Option<u8>,

    /// Request counter allows correlation between message sent and acknowledgment response from corrections stream
    #[clap(long)]
    request_counter: Option<u8>,
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

    CURL.with(|curl| -> Result<()> {
        let mut curl = curl.borrow_mut();

        let mut headers = List::new();
        let mut client_header = "X-SwiftNav-Client-Id: ".to_string();
        client_header.push_str(&opt.client_id);

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
            let now = if let Some(epoch) = opt.epoch {
                SystemTime::UNIX_EPOCH.add(Duration::from_secs(epoch.into()))
            } else {
                SystemTime::now()
            };
            let elapsed = LAST.with(|last| {
                let dur = now.duration_since(*last.borrow());
                dur.unwrap_or_else(|_| Duration::from_secs(0)).as_secs()
            });
            if opt.gga_period > 0 && elapsed > opt.gga_period {
                LAST.with(|last| *last.borrow_mut() = now);
                let datetime: DateTime<Utc> = now.into();
                let time = datetime.format("%H%M%S.00");
                let message = match &opt.area_id {
                    Some(area_id) => {
                        let request_counter = match &opt.request_counter {
                            Some(rc) => rc.to_string(),
                            None => String::new()
                        };
                        let solution_id = match &opt.solution_id {
                            Some(solution_id) => solution_id.to_string(),
                            None => String::new()
                        };
                        format!("$PSWTCRA,{},{},,{},", request_counter, area_id, solution_id)
                    },
                    None => {
                        format!("$GPGGA,{},{:02}{:010.7},{},{:03}{:010.7},{},4,12,1.3,{:.2},M,0.0,M,1.7,0078",
                        time, lat_deg, lat_min, lat_dir, lon_deg, lon_min, lon_dir, heightf)
                    }
                };
                let checksum = checksum(message.as_bytes());
                let message = format!("{}*{:X}\r\n", message, checksum);
                buf.write_all(message.as_bytes()).unwrap();
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
