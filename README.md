![CI](https://github.com/swift-nav/ntripping/workflows/CI/badge.svg)

[![pings-rs][pings-rs-img]][ntripping]

# [ntripping][ntripping]

A debug utility for monitoring and inspecting NTRIP. This utility uses the same
libraries and mechanisms as the Piksi Multi.

## Install a pre-built package

Visit [the releases page](https://github.com/swift-nav/ntripping/releases) to
find a pre-built package for your platform.

## Building from source.

Building these utilities requires Rust.  First [install
Rust](https://rustup.rs/) then to build and install, run the following from a
checkout of this repository:

```
cargo install --path .
```

## Usage

The `ntripping` utility has the following usage:

    ntripping 0.1.0
    NTRIP command line client.

    USAGE:
        ntripping [FLAGS] [OPTIONS]

    FLAGS:
        -h, --help       Prints help information
        -V, --version    Prints version information
        -v, --verbose

    OPTIONS:
            --client-id <client-id> [default: 00000000-0000-0000-0000-000000000000]
            --height <height>       [default: -5.549358852471994]
            --lat <lat>             [default: 37.77101999622968]
            --lon <lon>             [default: -122.40315159140708]
            --url <url>             [default: na.skylark.swiftnav.com:2101/CRS]

Different resources can be requested from different locations. By default, a San
Francisco latitude, longitude, and height will be used. The data returned is in RTCM format.

The `--url` must be formatted as such: `{username}:{passsword}@{area}.skylark.swiftnav.com:{port}/{mountpoint}`, 
where `username` and `password` refer to the user's [device registry](https://device-registry.cs.swiftnav.com/#/)
username and password. 

For example, if a user named John Doe has registed for a username of `john.doe` and password of `pa$$w0rd`
at the [device registry](https://device-registry.cs.swiftnav.com/#/) and wants to query for OSR data at
this location in North America: `(lat: 37.831235 deg, lon: -122.286484, height: -17.425m)`,
he will use the following command:
```
ntripping -v --height -17.425 --lat 37.831235 --lon -122.286484 --url john.doe:pa$$w0rd@na.skylark.swiftnav.com:2101/OSR
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
