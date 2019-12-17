pub const SOC_BASE_PATH: &'static str = "esp-idf/components/soc/esp32/include/soc/";

use header2svd::{REG_BASE, REG_BITS, REG_DEF, REG_DEF_INDEX};
use regex::Regex;
use std::fs::{DirEntry, File};
use std::io::prelude::*;

fn main() {
    // let registers = Hashmap::new();

    let filname = SOC_BASE_PATH.to_owned() + "soc.h";
    let re_base = Regex::new(REG_BASE).unwrap();
    let re_reg = Regex::new(REG_DEF).unwrap();
    let re_reg_index = Regex::new(REG_DEF_INDEX).unwrap();
    let re_reg_bits = Regex::new(REG_BITS).unwrap();
    

    for caps in re_base.captures_iter(file_to_string(&filname).as_str()) {
        // println!("Found: {:?}", caps);
        // TODO load up
    }

    std::fs::read_dir(SOC_BASE_PATH)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|f| f.path().to_str().unwrap().ends_with("_reg.h"))
        .for_each(|f| {
            let name = f.path();
            let name = name.to_str().unwrap();
            println!("Searching file: {}", name);
            let file_data = file_to_string(name);

            /* Normal register definitions */
            for register in re_reg.captures_iter(file_data.as_str()) {
                match (&register[1], &register[2], &register[3]) {
                    // TODO, running cargo run | grep "Normal" | grep "(i)", a few slip through, we need to handle that
                    (reg_name, pname, offset) => {
                        println!("Normal: {} @ {}", reg_name, offset);
                    }
                    _ => {}
                }
            }

            /* Indexed register definitions */
            for register in re_reg_index.captures_iter(file_data.as_str()) {
                match (&register[1], &register[2], &register[3]) {
                    (reg_name, pname, offset) => {
                        println!("Indexed: {} @ {}", reg_name, offset);
                    }
                    _ => {}
                }
            }

        });
}


fn file_to_string(fil: &str) -> String {
    let mut soc = File::open(fil).unwrap();
    let mut data = String::new();
    soc.read_to_string(&mut data).unwrap();
    data
}
