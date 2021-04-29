# header2svd

A tool for generating SVD files for the ESP32, ESP32-C3, and ESP8266. Uses [esp-idf] for the ESP32/ESP32-C3 and [ESP8266_RTOS_SDK] for the ESP8266.

This tool is required because official SVD files are not available for these devices at this time. The generated SVD files are used for generating the [esp32] and [esp8266] peripheral access crates using [svd2rust].

## Building

Clone the repository, [esp-idf], and [ESP8266_RTOS_SDK]. Move into the directory and build the application.

```bash
$ git clone --recursive https://github.com/MabezDev/idf2svd
$ cd idf2svd/ && cargo build
```

### ESP32/ESP32-C3

```bash
$ cargo run esp32
$ cargo run esp32c3
```

This will create either `esp32.svd` or `esp32c3.svd` in the base project directory.

### ESP8266

It is necessary to have `java`, `make`, `qpdf`, and `wget` installed on your system and available on `PATH`. These can generally be installed via your operating system's package manager.

```bash
$ # `make` only needs to be run once upon checking out the repository. It
$ # takes care of downloading some additional tools and resources for
$ # generating the ESP8266 SVD.
$ make
$ cargo run esp8266
```

This will create the file `esp8266.svd` in the base project directory.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

[esp-idf]: https://github.com/espressif/esp-idf
[esp8266_rtos_sdk]: https://github.com/espressif/ESP8266_RTOS_SDK
[esp32]: https://github.com/esp-rs/esp32
[esp8266]: https://github.com/esp-rs/esp8266
[svd2rust]: https://github.com/rust-embedded/svd2rust
