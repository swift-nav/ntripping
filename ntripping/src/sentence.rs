use std::fmt::Write;

use bytes::Bytes;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy)]
pub enum Sentence {
    GGA(Gga),
    CRA(Cra),
}

impl Sentence {
    pub fn with_time(mut self, time: impl Into<Option<DateTime<Utc>>>) -> Self {
        if let Self::GGA(ref mut gga) = self {
            gga.time = time.into()
        }
        self
    }

    pub fn with_request_counter(mut self, request_counter: impl Into<Option<u8>>) -> Self {
        if let Self::CRA(ref mut cra) = self {
            cra.request_counter = request_counter.into()
        }
        self
    }

    pub(crate) fn to_string(self, carriage_return: bool) -> String {
        let mut buf = String::from('$');
        match self {
            Sentence::GGA(g) => g.write_string(&mut buf),
            Sentence::CRA(c) => c.write_string(&mut buf),
        }
        write!(buf, "*{:X}", checksum(buf.as_bytes())).unwrap();
        if carriage_return {
            buf.push_str("\r\n");
        }
        buf
    }
}

impl From<Gga> for Sentence {
    fn from(gga: Gga) -> Self {
        Self::GGA(gga)
    }
}

impl From<Cra> for Sentence {
    fn from(cra: Cra) -> Self {
        Self::CRA(cra)
    }
}

impl From<Sentence> for Bytes {
    fn from(s: Sentence) -> Self {
        Bytes::from(s.to_string(true))
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Gga {
    time: Option<DateTime<Utc>>,
    lat: Option<f64>,
    lon: Option<f64>,
    fix_type: Option<u8>,
    num_satellites: Option<u8>,
    hdop: Option<f64>,
    height: Option<f64>,
    geoid_height: Option<f64>,
    age_of_corrections: Option<f64>,
    station_id: Option<u16>,
}

impl Gga {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_time(mut self, time: impl Into<Option<DateTime<Utc>>>) -> Self {
        self.time = time.into();
        self
    }

    pub fn with_lat(mut self, lat: impl Into<Option<f64>>) -> Self {
        self.lat = lat.into();
        self
    }

    pub fn with_lon(mut self, lon: impl Into<Option<f64>>) -> Self {
        self.lon = lon.into();
        self
    }

    pub fn with_fix_type(mut self, fix_type: impl Into<Option<u8>>) -> Self {
        self.fix_type = fix_type.into();
        self
    }

    pub fn with_num_satellites(mut self, num_satellites: impl Into<Option<u8>>) -> Self {
        self.num_satellites = num_satellites.into();
        self
    }

    pub fn with_hdop(mut self, hdop: impl Into<Option<f64>>) -> Self {
        self.hdop = hdop.into();
        self
    }

    pub fn with_height(mut self, height: impl Into<Option<f64>>) -> Self {
        self.height = height.into();
        self
    }

    pub fn with_geoid_height(mut self, geoid_height: impl Into<Option<f64>>) -> Self {
        self.geoid_height = geoid_height.into();
        self
    }

    pub fn with_age_of_corrections(mut self, age_of_corrections: impl Into<Option<f64>>) -> Self {
        self.age_of_corrections = age_of_corrections.into();
        self
    }

    pub fn with_station_id(mut self, station_id: impl Into<Option<u16>>) -> Self {
        self.station_id = station_id.into();
        self
    }

    pub(crate) fn write_string(&self, buf: &mut String) {
        buf.push_str("GPGGA,");

        if let Some(time) = self.time {
            write!(buf, "{},", time.format("%H%M%S.00")).unwrap();
        } else {
            buf.push(',');
        }

        if let Some(lat) = self.lat {
            let latn = ((lat * 1e8).round() / 1e8).abs();
            let lat_deg = latn as u16;
            let lat_min = (latn - (lat_deg as f64)) * 60.0;
            let lat_dir = if lat < 0.0 { 'S' } else { 'N' };
            write!(buf, "{lat_deg:02}{lat_min:010.7},{lat_dir},").unwrap()
        } else {
            buf.push_str(",,");
        }

        if let Some(lon) = self.lon {
            let lonn = ((lon * 1e8).round() / 1e8).abs();
            let lon_deg = lonn as u16;
            let lon_min = (lonn - (lon_deg as f64)) * 60.0;
            let lon_dir = if lon < 0.0 { 'W' } else { 'E' };
            write!(buf, "{lon_deg:03}{lon_min:010.7},{lon_dir},").unwrap()
        } else {
            buf.push_str(",,");
        }

        if let Some(fix_type) = self.fix_type {
            write!(buf, "{fix_type},").unwrap();
        } else {
            buf.push(',');
        }

        if let Some(satellites) = self.num_satellites {
            write!(buf, "{satellites},").unwrap();
        } else {
            buf.push(',');
        }

        if let Some(hdop) = self.hdop {
            write!(buf, "{hdop:.1},").unwrap();
        } else {
            buf.push(',');
        }

        if let Some(height) = self.height {
            write!(buf, "{height:.2},M,").unwrap();
        } else {
            buf.push_str(",M,");
        }

        if let Some(geoid_height) = self.geoid_height {
            write!(buf, "{geoid_height:.1},M,").unwrap();
        } else {
            buf.push_str(",M,");
        }

        if let Some(age_of_corrections) = self.age_of_corrections {
            write!(buf, "{age_of_corrections:.1},").unwrap();
        } else {
            buf.push(',');
        }

        if let Some(corrections_station_id) = self.station_id {
            write!(buf, "{corrections_station_id:04}").unwrap();
        }
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Cra {
    request_counter: Option<u8>,
    area_id: Option<u32>,
    corrections_mask: Option<u16>,
    solution_id: Option<u8>,
}

impl Cra {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_request_counter(mut self, request_counter: impl Into<Option<u8>>) -> Self {
        self.request_counter = request_counter.into();
        self
    }

    pub fn with_area_id(mut self, area_id: impl Into<Option<u32>>) -> Self {
        self.area_id = area_id.into();
        self
    }

    pub fn with_corrections_mask(mut self, corrections_mask: impl Into<Option<u16>>) -> Self {
        self.corrections_mask = corrections_mask.into();
        self
    }

    pub fn with_solution_id(mut self, solution_id: impl Into<Option<u8>>) -> Self {
        self.solution_id = solution_id.into();
        self
    }

    pub(crate) fn write_string(&self, buf: &mut String) {
        buf.push_str("PSWTCRA,");

        if let Some(request_counter) = self.request_counter {
            write!(buf, "{request_counter},").unwrap();
        } else {
            buf.push(',');
        }

        if let Some(area_id) = self.area_id {
            write!(buf, "{area_id},").unwrap();
        } else {
            buf.push(',');
        }
        if let Some(corrections_mask) = self.corrections_mask {
            write!(buf, "{corrections_mask},").unwrap();
        } else {
            buf.push(',');
        }

        if let Some(solution_id) = self.solution_id {
            write!(buf, "{solution_id}").unwrap();
        }
    }
}

fn checksum(buf: &[u8]) -> u8 {
    let mut sum: u8 = 0;
    for c in &buf[1..] {
        sum ^= c;
    }
    sum
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn gga() {
        let test_data = [
            (Gga::new(), "$GPGGA,,,,,,,,,,M,,M,,*56"),
            (
                Gga::new()

                    .with_time(Utc.with_ymd_and_hms(2020, 1, 1, 18, 59, 40).unwrap())
                    .with_lat(37.77103777)
                    .with_lon(-122.40316335)
                    .with_fix_type(5)
                    .with_num_satellites(10)
                    .with_hdop(0.9)
                    .with_height(-8.09)
                    .with_geoid_height(0.0)
                    .with_age_of_corrections(1.3)
                    .with_station_id(0),
                "$GPGGA,185940.00,3746.2622662,N,12224.1898010,W,5,10,0.9,-8.09,M,0.0,M,1.3,0000*7D",
            ),
        ].map(|(gga, expected)| (Sentence::from(gga), expected.to_string()));

        for (gga, expected) in test_data {
            assert_eq!(gga.to_string(false), expected);
        }
    }

    #[test]
    fn cra() {
        let test_data = [
            (Cra::new(), "$PSWTCRA,,,,*50"),
            (Cra::new().with_request_counter(0), "$PSWTCRA,0,,,*60"),
            (Cra::new().with_area_id(0), "$PSWTCRA,,0,,*60"),
            (Cra::new().with_corrections_mask(0), "$PSWTCRA,,,0,*60"),
            (Cra::new().with_solution_id(0), "$PSWTCRA,,,,0*60"),
            (
                Cra::new()
                    .with_area_id(0)
                    .with_request_counter(0)
                    .with_corrections_mask(0)
                    .with_solution_id(0),
                "$PSWTCRA,0,0,0,0*50",
            ),
        ]
        .map(|(gga, expected)| (Sentence::from(gga), expected.to_string()));

        for (gga, expected) in test_data {
            assert_eq!(gga.to_string(false), expected);
        }
    }
}
