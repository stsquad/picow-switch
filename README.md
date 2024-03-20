PicoW Switch
============

This is a project to build a simple Pico W controlled relay switch
that interfaces with Home Assistant over MQTT. It is built using the
[Embassy](https://github.com/embassy-rs/embassy) framework for
embedded Rust devices.

I'm using a [SB single channel relay
HAT](https://learn.sb-components.co.uk/Pico-Single-Channel-Relay-Hat)
for convenience but any GPIO controlled device will do.

Building
--------

This uses the [embassy-rs](https://github.com/embassy-rs/embassy)
framework for building and that recommends having `probe-rs`
installed:

    cargo install probe-rs --features cli

This allows the binary to be uploaded to the device through a devprobe
by calling:

    cargo run --release --bin picow-switch

Configuration
-------------

The network and MQTT details are embedded in the config.rs file.
Please fill in with the details of your network when 
