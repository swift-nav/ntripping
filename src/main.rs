use std::cell::RefCell;
use std::fmt::Write;
use std::io::{self, Write as _};
use std::iter;
use std::path::PathBuf;
use std::rc::Rc;
use std::thread;
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Utc};
use clap::{ArgGroup, Parser};
use curl::easy::{Easy, HttpVersion, List, ReadError};
use flume::TryRecvError;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

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
    #[arg(long, default_value = "na.skylark.swiftnav.com:2101/")]
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

    /// NMEA sentence update period, in seconds. 0 means to never send a sentence
    #[arg(
        long,
        default_value_t = 10,
        conflicts_with = "input",
        alias = "gga-period"
    )]
    nmea_period: u64,

    /// Send the NMEA sentence in the HTTP header
    #[arg(long)]
    nmea_header: bool,

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
    #[serde(default = "default_after")]
    after: u64,
    epoch: Option<u32>,
    crc: Option<u8>,
    #[serde(flatten)]
    message: Message,
}

fn default_after() -> u64 {
    10
}

impl Command {
    fn to_bytes(self) -> Vec<u8> {
        self.to_string().into_bytes()
    }
}

impl std::fmt::Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let now = self.epoch.map_or_else(SystemTime::now, |e| {
            SystemTime::UNIX_EPOCH + Duration::from_secs(e.into())
        });
        let message = self.message.format(now.into());
        let checksum = self.crc.unwrap_or_else(|| checksum(message.as_bytes()));
        write!(f, "{message}*{checksum:X}")
    }
}

fn checksum(buf: &[u8]) -> u8 {
    let mut sum = 0;
    for c in &buf[1..] {
        sum ^= c;
    }
    sum
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

fn build_cra(opt: &Cli) -> Command {
    Command {
        epoch: opt.epoch,
        after: 0,
        crc: None,
        message: Message::Cra {
            request_counter: opt.request_counter,
            area_id: opt.area_id,
            corrections_mask: opt.corrections_mask,
            solution_id: opt.solution_id,
        },
    }
}

fn build_gga(opt: &Cli) -> Command {
    Command {
        epoch: opt.epoch,
        after: 0,
        crc: None,
        message: Message::Gga {
            lat: opt.lat,
            lon: opt.lon,
            height: opt.height,
        },
    }
}

fn get_commands(opt: Cli) -> Result<Box<dyn Iterator<Item = Command> + Send>> {
    if let Some(path) = opt.input {
        let file = std::fs::File::open(path)?;
        let cmds: Vec<_> = serde_yaml::from_reader(file)?;
        return Ok(Box::new(cmds.into_iter()));
    }

    if opt.nmea_period == 0 {
        return Ok(Box::new(iter::empty()));
    }

    if opt.area_id.is_some() {
        let first = build_cra(&opt);
        let it = iter::successors(Some(first), move |prev| {
            let mut next = *prev;
            if let Message::Cra {
                request_counter: Some(ref mut counter),
                ..
            } = &mut next.message
            {
                *counter = counter.wrapping_add(1);
            }
            next.after = opt.nmea_period;
            Some(next)
        });
        Ok(Box::new(it))
    } else {
        let first = build_gga(&opt);
        let rest = iter::repeat(Command {
            after: opt.nmea_period,
            ..first
        });
        Ok(Box::new(iter::once(first).chain(rest)))
    }
}

fn run() -> Result<()> {
    let opt = Cli::parse();

    let mut curl = Easy::new();

    let mut headers = List::new();
    headers.append("Transfer-Encoding:")?;
    headers.append("Ntrip-Version: Ntrip/2.0")?;
    headers.append(&format!("X-SwiftNav-Client-Id: {}", opt.client_id))?;

    if opt.nmea_header {
        if opt.area_id.is_some() {
            headers.append(&format!("Ntrip-CRA: {}", build_cra(&opt)))?;
        } else {
            headers.append(&format!("Ntrip-GGA: {}", build_gga(&opt)))?;
        }
    }

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

    let (tx, rx) = flume::bounded::<Vec<u8>>(1);
    let transfer = Rc::new(RefCell::new(curl.transfer()));

    transfer.borrow_mut().progress_function({
        let rx = &rx;
        let transfer = Rc::clone(&transfer);
        move |_dltot, _dlnow, _ultot, _ulnow| {
            if !rx.is_empty() {
                if let Err(e) = transfer.borrow().unpause_read() {
                    eprintln!("unpause error: {e}");
                    return false;
                }
            }
            true
        }
    })?;

    transfer.borrow_mut().write_function(|data| {
        if let Err(e) = io::stdout().write_all(data) {
            eprintln!("write error: {e}");
            return Ok(0);
        }
        Ok(data.len())
    })?;

    transfer.borrow_mut().read_function(|mut data: &mut [u8]| {
        let mut bytes = match rx.try_recv() {
            Ok(bytes) => bytes,
            Err(TryRecvError::Empty) => return Err(ReadError::Pause),
            Err(TryRecvError::Disconnected) => return Err(ReadError::Abort),
        };
        bytes.extend_from_slice(b"\r\n");
        if let Err(e) = data.write_all(&bytes) {
            eprintln!("read error: {e}");
            return Err(ReadError::Abort);
        }
        Ok(bytes.len())
    })?;

    let commands = get_commands(opt.clone())?;
    let handle = thread::spawn(move || {
        for cmd in commands {
            if cmd.after > 0 {
                thread::sleep(Duration::from_secs(cmd.after));
            }
            if tx.send(cmd.to_bytes()).is_err() {
                break;
            }
        }
        Ok(())
    });

    transfer.borrow().perform()?;

    if !handle.is_finished() {
        Ok(())
    } else {
        // an error stopped the thread early
        handle.join().unwrap()
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e}");
    }
}
