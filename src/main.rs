use std::cell::RefCell;
use std::fmt::Write;
use std::io::{self, Write as _};
use std::iter;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering::SeqCst},
    mpsc, Arc,
};
use std::thread;
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Utc};
use clap::{ArgGroup, Parser};
use curl::easy::{Easy, HttpVersion, List, ReadError};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

thread_local! {
    static CURL: RefCell<Easy> = RefCell::new(Easy::new());
}

#[derive(Debug, Clone, Parser)]
#[command(
    name = "ntripping",
    about = "NTRIP command line client",
    version = env!("VERGEN_SEMVER_LIGHTWEIGHT"),
    group(
        ArgGroup::new("gga")
            .conflicts_with_all(["input", "cra"])
            .args(["lat", "lon", "height"])
            .multiple(true),
    ),
    group(
        ArgGroup::new("cra")
            .conflicts_with_all(["input", "gga"])
            .args(["request_counter", "area_id", "corrections_mask", "solution_id"])
            .multiple(true),
    ),
)]
struct Cli {
    /// URL of the NTRIP caster
    #[arg(long, default_value = "na.skylark.swiftnav.com:2101/CRS")]
    url: String,

    /// Receiver latitude to report, in degrees
    #[arg(long, default_value_t = 37.77101999622968, allow_hyphen_values = true)]
    lat: f64,

    /// Receiver longitude to report, in degrees
    #[arg(long, default_value_t = -122.40315159140708, allow_hyphen_values = true)]
    lon: f64,

    /// Receiver height to report, in meters
    #[arg(long, default_value_t = -5.549358852471994, allow_hyphen_values = true)]
    height: f64,

    /// Client ID
    #[arg(
        long,
        default_value = "00000000-0000-0000-0000-000000000000",
        alias = "client"
    )]
    client_id: String,

    #[arg(short, long)]
    verbose: bool,

    /// Receiver time to report, as a Unix time
    #[arg(long)]
    epoch: Option<u32>,

    /// Username credentials
    #[arg(long)]
    username: Option<String>,

    /// Password credentials
    #[arg(long)]
    password: Option<String>,

    /// GGA update period, in seconds. 0 means to never send a GGA
    #[arg(long, default_value_t = 10)]
    gga_period: u64,

    /// Request counter allows correlation between message sent and acknowledgment response from corrections stream
    #[arg(long)]
    request_counter: Option<u8>,

    /// Area ID to be used in generation of CRA message. If this flag is set, ntripping outputs messages of type CRA rather than the default GGA
    #[arg(long)]
    area_id: Option<u32>,

    /// Field specifying which types of corrections are to be received
    #[arg(long)]
    corrections_mask: Option<u16>,

    /// Solution ID, the identifier of the connection stream to reconnect to in the event of disconnections
    #[arg(long)]
    solution_id: Option<u8>,

    /// Path to a YAML file containing a list of messages to send to the caster
    #[arg(long)]
    input: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
struct Command {
    epoch: Option<u32>,
    after: Option<u64>,
    crc: Option<u8>,
    #[serde(flatten)]
    message: Message,
}

impl Command {
    fn to_bytes(&self) -> Vec<u8> {
        let now = self.epoch.map_or_else(SystemTime::now, |e| {
            SystemTime::UNIX_EPOCH + Duration::from_secs(e.into())
        });
        let message = self.message.format(now.into());
        let checksum = self.crc.unwrap_or_else(|| checksum(message.as_bytes()));
        let message = format!("{message}*{checksum:X}\r\n");
        message.into_bytes()
    }
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum Message {
    Gga {
        lat: f64,
        lon: f64,
        height: f64,
    },
    Cra {
        request_counter: Option<u8>,
        area_id: Option<u32>,
        corrections_mask: Option<u16>,
        solution_id: Option<u8>,
    },
}

impl Message {
    fn format(&self, time: DateTime<Utc>) -> String {
        match *self {
            Message::Gga { lat, lon, height } => {
                let time = time.format("%H%M%S.00");

                let latn = ((lat * 1e8).round() / 1e8).abs();
                let lonn = ((lon * 1e8).round() / 1e8).abs();

                let lat_deg = latn as u16;
                let lon_deg = lonn as u16;

                let lat_min = (latn - (lat_deg as f64)) * 60.0;
                let lon_min = (lonn - (lon_deg as f64)) * 60.0;

                let lat_dir = if lat < 0.0 { 'S' } else { 'N' };
                let lon_dir = if lon < 0.0 { 'W' } else { 'E' };

                format!(
                    "$GPGGA,{},{:02}{:010.7},{},{:03}{:010.7},{},4,12,1.3,{:.2},M,0.0,M,1.7,0078",
                    time, lat_deg, lat_min, lat_dir, lon_deg, lon_min, lon_dir, height
                )
            }
            Message::Cra {
                request_counter,
                area_id,
                corrections_mask,
                solution_id,
            } => {
                let mut s = String::from("$PSWTCRA,");
                if let Some(request_counter) = request_counter {
                    write!(&mut s, "{request_counter}").unwrap();
                }
                s.push(',');
                if let Some(area_id) = area_id {
                    write!(&mut s, "{area_id}").unwrap();
                }
                s.push(',');
                if let Some(corrections_mask) = corrections_mask {
                    write!(&mut s, "{corrections_mask}").unwrap();
                }
                s.push(',');
                if let Some(solution_id) = solution_id {
                    write!(&mut s, "{solution_id}").unwrap();
                }
                s
            }
        }
    }
}

fn get_commands(opt: &Cli) -> Result<Box<dyn Iterator<Item = Command> + Send>> {
    if opt.gga_period == 0 && opt.input.is_none() {
        return Ok(Box::new(iter::empty()));
    }
    match opt.clone() {
        Cli {
            input: Some(path), ..
        } => {
            let file = std::fs::File::open(path)?;
            let cmds: Vec<_> = serde_yaml::from_reader(file)?;
            Ok(Box::new(cmds.into_iter()))
        }
        opt if opt.area_id.is_some() => {
            let first = Command {
                epoch: opt.epoch,
                after: None,
                crc: None,
                message: Message::Cra {
                    request_counter: opt.request_counter,
                    area_id: opt.area_id,
                    corrections_mask: opt.corrections_mask,
                    solution_id: opt.solution_id,
                },
            };
            let it = iter::successors(Some(first), move |prev| {
                let mut next = *prev;
                next.after = Some(opt.gga_period);
                if let Message::Cra {
                    request_counter: Some(ref mut counter),
                    ..
                } = &mut next.message
                {
                    *counter = counter.wrapping_add(1);
                }
                Some(next)
            });
            Ok(Box::new(it))
        }
        opt => {
            let first = Command {
                epoch: opt.epoch,
                after: None,
                crc: None,
                message: Message::Gga {
                    lat: opt.lat,
                    lon: opt.lon,
                    height: opt.height,
                },
            };
            let rest = iter::repeat(Command {
                after: Some(opt.gga_period),
                ..first
            });
            Ok(Box::new(iter::once(first).chain(rest)))
        }
    }
}

fn checksum(buf: &[u8]) -> u8 {
    let mut sum = 0;
    for c in &buf[1..] {
        sum ^= c;
    }
    sum
}

fn main() -> Result<()> {
    let opt = Cli::parse();

    let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(1);
    let ready = Arc::new(AtomicBool::new(true));
    let cmds = get_commands(&opt)?;

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

        curl.progress_function({
            let ready = Arc::clone(&ready);
            move |_dltot, _dlnow, _ultot, _ulnow| {
                if ready.swap(false, SeqCst) {
                    if let Err(e) = CURL.with(|curl| curl.borrow().unpause_read()) {
                        eprintln!("unpause error: {e}");
                        return false;
                    }
                }
                true
            }
        })?;

        curl.read_function(move |mut buf: &mut [u8]| {
            let Ok(bytes) = rx.try_recv() else {
                return Err(ReadError::Pause);
            };
            if let Err(e) = buf.write_all(&bytes) {
                eprintln!("write error: {e}");
                return Err(ReadError::Abort);
            }
            Ok(bytes.len())
        })?;

        Ok(())
    })?;

    if atty::is(atty::Stream::Stdin) {
        thread::spawn(move || {
            for cmd in cmds {
                if let Some(d) = cmd.after {
                    thread::sleep(Duration::from_secs(d));
                }
                if tx.send(cmd.to_bytes()).is_err() {
                    break;
                }
                ready.store(true, SeqCst);
            }
        });
    } else {
        thread::spawn(move || {
            let stdin = io::stdin().lock();
            for line in io::BufRead::lines(stdin) {
                if tx.send(line.unwrap().into_bytes()).is_err() {
                    break;
                }
                ready.store(true, SeqCst);
            }
        });
    };

    CURL.with(|curl| curl.borrow().perform())?;

    Ok(())
}
