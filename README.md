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
            --height <height>     [default: -5.549358852471994]
            --lat <lat>           [default: 37.77101999622968]
            --lon <lon>           [default: -122.40315159140708]
            --url <url>           [default: na.skylark.swiftnav.com:2101/CRS]

Different resources can be requested from different locations. By default, a San
Francisco latitude, longitude, and height will be used.

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
