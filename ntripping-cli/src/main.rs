use std::time::{Duration, SystemTime};

use chrono::{DateTime, Utc};
use clap::Parser;
use futures_util::{SinkExt, TryStreamExt};
use tokio::io::{self, AsyncWriteExt};
use tracing_subscriber::filter::LevelFilter;

use ntripping::{
    sentence::{Cra, Gga, Sentence},
    Auth, Client,
};

#[derive(Debug, clap::Parser)]
#[clap(
    name = "ntripping",
    about = "NTRIP command line client.",
    version = env!("VERGEN_SEMVER_LIGHTWEIGHT"),
)]
struct Cli {
    /// URL of the NTRIP caster
    #[clap(long, default_value = "na.skylark.swiftnav.com:2101")]
    url: String,

    /// Receiver latitude to report, in degrees
    #[clap(long, default_value_t = 37.77101999622968, allow_hyphen_values = true)]
    lat: f64,

    /// Receiver longitude to report, in degrees
    #[clap(
        long,
        default_value_t = -122.40315159140708,
        allow_hyphen_values = true
    )]
    lon: f64,

    /// Receiver height to report, in meters
    #[clap(long, default_value_t = -5.549358852471994, allow_hyphen_values = true)]
    height: f64,

    /// Client ID
    #[clap(
        long,
        default_value = "00000000-0000-0000-0000-000000000000",
        alias = "client"
    )]
    client_id: String,

    /// Verbosity level, can be specified multiple times
    #[clap(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

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
    #[clap(long, default_value_t = 10)]
    gga_period: u64,

    /// Set the ntrip-gga header
    #[clap(long)]
    gga_header: bool,

    /// Request counter allows correlation between message sent and acknowledgment response from corrections stream
    #[clap(long, default_value_t = 0)]
    request_counter: u8,

    /// Area ID to be used in generation of CRA message. If this flag is set, ntripping outputs messages of type CRA rather than the default GGA
    #[clap(long)]
    area_id: Option<u32>,

    /// Field specifying which types of corrections are to be received
    #[clap(long)]
    corrections_mask: Option<u16>,

    /// Solution ID, the identifier of the connection stream to reconnect to in the event of disconnections
    #[clap(long)]
    solution_id: Option<u8>,
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

async fn run(opt: Cli) -> Result<()> {
    let now = || -> DateTime<Utc> {
        if let Some(epoch) = opt.epoch {
            SystemTime::UNIX_EPOCH + Duration::from_secs(epoch.into())
        } else {
            SystemTime::now()
        }
        .into()
    };

    let auth = if let (Some(username), Some(password)) = (opt.username, opt.password) {
        Some(Auth::new(username, password))
    } else {
        None
    };

    let msg: Sentence = if let Some(area_id) = opt.area_id {
        Cra::new()
            .with_area_id(area_id)
            .with_corrections_mask(opt.corrections_mask)
            .with_solution_id(opt.solution_id)
            .into()
    } else {
        Gga::new()
            .with_time(now())
            .with_lat(opt.lat)
            .with_lon(opt.lon)
            .with_height(opt.height)
            .into()
    };

    let client = {
        let client = Client::new().with_client_id(opt.client_id).with_auth(auth);
        if opt.gga_header {
            client.with_ntrip_gga(msg)
        } else {
            client
        }
    };

    let (mut sink, mut stream) = {
        let url = if !opt.url.starts_with("http://") && !opt.url.starts_with("https://") {
            format!("https://{}", opt.url)
        } else {
            opt.url
        };
        let uri = url.parse()?;
        client.connect(uri).await?.split()
    };

    let writer_task = tokio::spawn(async move {
        let mut out = io::stdout();
        while let Some(data) = stream.try_next().await? {
            out.write_all(&data).await?;
        }
        Result::Ok(())
    });

    if opt.gga_period == 0 {
        return writer_task.await?;
    }

    let mut request_counter = opt.request_counter;
    let mut gga_interval = tokio::time::interval(Duration::from_secs(opt.gga_period));
    loop {
        if writer_task.is_finished() {
            break;
        }
        gga_interval.tick().await;
        let sentence = msg.with_time(now()).with_request_counter(request_counter);
        let _ = sink.send(sentence).await;
        request_counter = request_counter.wrapping_add(1);
    }

    Ok(())
}

fn main() -> Result<()> {
    let opt = Cli::parse();
    tracing_subscriber::fmt::fmt()
        .with_max_level(match opt.verbose {
            0 => LevelFilter::WARN,
            1 => LevelFilter::DEBUG,
            _ => LevelFilter::TRACE,
        })
        .with_writer(std::io::stderr)
        .compact()
        .init();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(run(opt))
}
