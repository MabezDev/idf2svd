pub const SOC_BASE_PATH: &'static str = "esp-idf/components/soc/esp32/include/soc/";

use header2svd::{REG_BASE, REG_BITS, REG_DEF, REG_DEF_INDEX, Peripheral, Register, REG_DESC};
use regex::Regex;
use std::fs::File;
use std::io::prelude::*;
use std::collections::HashMap;

fn main() {
    let mut peripherals = HashMap::new();

    let filname = SOC_BASE_PATH.to_owned() + "soc.h";
    let re_base = Regex::new(REG_BASE).unwrap();
    let re_reg = Regex::new(REG_DEF).unwrap();
    let re_reg_index = Regex::new(REG_DEF_INDEX).unwrap();
    let re_reg_desc = Regex::new(REG_DESC).unwrap();
    let re_reg_bits = Regex::new(REG_BITS).unwrap();
    

    for captures in re_base.captures_iter(file_to_string(&filname).as_str()) {
        let peripheral = &captures[1];
        let address = &captures[2];
        let mut p = Peripheral::default();
        p.address = u32::from_str_radix(address, 16).unwrap();
        p.description = peripheral.to_string();

        peripherals.insert(peripheral.to_string(), p);
    }

    std::fs::read_dir(SOC_BASE_PATH)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|f| f.path().to_str().unwrap().ends_with("_reg.h"))
        .for_each(|f| {
            let name = f.path();
            let name = name.to_str().unwrap();
            // println!("Searching file: {}", name);
            let file_data = file_to_string(name);

            /* Normal register definitions */
            for register in re_reg.captures_iter(file_data.as_str()) {
                let reg_name = &register[1];
                let pname = &register[2];
                let offset = &register[3].trim_start_matches("0x");
                if reg_name.ends_with("(i)") { // some indexed still get through, ignore them
                    continue;
                }
                if let Ok(addr) = u32::from_str_radix(offset, 16) {
                    let p = peripherals.get_mut(&pname.to_string()).unwrap();
                    let mut r = Register::default();
                    r.description = reg_name.to_string();
                    r.name = reg_name.to_string();
                    r.address = addr;
                    p.registers.push(r);
                } else {
                    println!("Failed to parse register for {}: {}", reg_name, offset)
                }
            }

            /* Indexed register definitions */
            for register in re_reg_index.captures_iter(file_data.as_str()) {
                let reg_name = &register[1];
                let pname = &register[2];
                let offset = &register[3].trim_start_matches("0x");
                if let Ok(addr) = u32::from_str_radix(offset, 16) {
                    if let Some(p) = peripherals.get_mut(&pname.to_string()) {
                        let mut r = Register::default();
                        r.description = reg_name.to_string();
                        r.name = reg_name.to_string();
                        r.address = addr;
                        p.registers.push(r);
                    } else {
                        println!("No periphal called {}", pname.to_string());
                    }
                } else {
                    println!("Failed to parse register for {}: {}", reg_name, offset)
                }
            }

        });

        // println!("{:#?}", peripherals);
}


fn file_to_string(fil: &str) -> String {
    let mut soc = File::open(fil).unwrap();
    let mut data = String::new();
    soc.read_to_string(&mut data).unwrap();
    data
}
