use clap::{app_from_crate, Arg};

mod idf;
mod sdk;

fn main() {
    let matches = app_from_crate!("\n")
        .arg(
            Arg::with_name("CHIP")
                .help("select which device to target")
                .required(true)
                .index(1)
                .possible_values(&["ESP32", "ESP8266"])
                .case_insensitive(true),
        )
        .get_matches();

    // Based on which chip has been selected, invoke the appropriate SVD
    // builder (since the ESP32 and ESP8266 have different SDKs).
    let chip = matches.value_of("CHIP").unwrap().to_uppercase();
    match chip.as_str() {
        "ESP32" => idf::create_svd(),
        "ESP8266" => sdk::create_svd(),
        _ => unimplemented!(),
    }
}
