# `idf2svd`

A tool for creating svd files from the [esp-idf](https://github.com/espressif/esp-idf) for the esp32. Currently targets the v4 release of esp idf.

## Building

Run `git clone --recursive https://github.com/MabezDev/idf2svd` to clone the repo and esp-idf, then execute
```
$ cd idf2svd && cargo run
```

this will emit esp32.svd which can be used to generate register access through [svd2rust](https://github.com/rust-embedded/svd2rust)