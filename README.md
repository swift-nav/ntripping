# [ntripping][ntripping] ![CI](https://github.com/swift-nav/ntripping/workflows/CI/badge.svg)

A debug utility for monitoring and inspecting NTRIP. This utility uses the same
libraries and mechanisms as the Piksi Multi.  Alternate version of [pings](https://github.com/swift-nav/pings)
which provides pre-built binaries.

[![pings-rs][pings-rs-img]][ntripping]

## Install a pre-built package

Visit [the releases page](https://github.com/swift-nav/ntripping/releases) to
find a pre-built package for your platform.

## Building from source.

Building these utilities requires Rust.  First [install
Rust](https://rustup.rs/) then to build and install, run the following from a
checkout of this repository:

```
cargo install --git https://github.com/swift-nav/ntripping.git
```

## Usage

The `ntripping` utility has the following usage:

    ntripping vX.Y.Z
    NTRIP command line client.

    USAGE:
        ntripping [OPTIONS]

    OPTIONS:
            --client <CLIENT>        Client ID [default: 00000000-0000-0000-0000-000000000000]
            --epoch <EPOCH>          Receiver time to report, as a Unix time
        -h, --help                   Print help information
            --height <HEIGHT>        Receiver height to report, in meters [default: -5.549358852471994]
            --lat <LAT>              Receiver latitude to report, in degrees [default:
                                     37.77101999622968]
            --lon <LON>              Receiver longitude to report, in degrees [default:
                                     -122.40315159140708]
            --password <PASSWORD>    Password credentials
            --url <URL>              URL of the NTRIP caster [default: na.skylark.swiftnav.com:2101/CRS]
            --username <USERNAME>    Username credentials
        -v, --verbose
        -V, --version                Print version information

Different resources can be requested from different locations. By default, a San
Francisco latitude, longitude, and height will be used.

### Credentials

Access credentials are usually required to access NTRIP streams. These credentials can be
specified individually as command line arguments or directly in the URL like this

```
ntripping --url user:pass@na.skylark.swiftnav.com:2101/CRS
```

## Copyright

```
Copyright (C) 2020 Swift Navigation Inc.
Contact: Swift Navigation <dev@swiftnav.com>

This source is subject to the license found in the file 'LICENSE' which must be
be distributed together with this source. All other rights reserved.

THIS CODE AND INFORMATION IS PROVIDED "AS IS" WITHOUT WARRANTY OF ANY KIND,
EITHER EXPRESSED OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE IMPLIED
WARRANTIES OF MERCHANTABILITY AND/OR FITNESS FOR A PARTICULAR PURPOSE.
```

[ntripping]: https://github.com/swift-nav/ntripping
[pings-rs-img]: ./img/pings-rs.png
