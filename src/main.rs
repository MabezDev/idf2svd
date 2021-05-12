use std::str::FromStr;

use clap::{app_from_crate, Arg};
use common::ChipType;

mod common;
mod idf;
mod sdk;

fn main() {
    let matches = app_from_crate!("\n")
        .arg(
            Arg::with_name("CHIP")
                .help("which device's SVD to generate")
                .required(true)
                .index(1)
                .possible_values(&["ESP32", "ESP8266", "ESP32C3"])
                .case_insensitive(true),
        )
        .get_matches();

    // Based on which chip has been selected, invoke the appropriate SVD
    // builder (since the ESP32 and ESP8266 have different SDKs).
    let chip = matches.value_of("CHIP").unwrap().to_uppercase();
    let chip = ChipType::from_str(&chip);
    match chip {
        Ok(chip) => match chip {
            ChipType::ESP32 => idf::create_svd(chip),
            ChipType::ESP32C3 => idf::create_svd(chip),
            ChipType::ESP8266 => sdk::create_svd(),
        },
        Err(e) => println!("{}", e),
    }
}
