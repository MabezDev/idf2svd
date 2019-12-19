pub const SOC_BASE_PATH: &'static str = "esp-idf/components/soc/esp32/include/soc/";

use header2svd::{
    BitField, Bits, Peripheral, Register, REG_BASE, REG_BITS, REG_BIT_INFO, REG_DEF, REG_DEF_INDEX,
    REG_DESC,
};
use regex::Regex;
use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;

enum State {
    FindReg,
    FindBitFieldInfo(String, Register),
    FindDescription(String, Register, BitField),
    CheckEnd(String, Register),
}

fn main() {
    let mut peripherals = HashMap::new();

    let filname = SOC_BASE_PATH.to_owned() + "soc.h";
    let re_base = Regex::new(REG_BASE).unwrap();
    let re_reg = Regex::new(REG_DEF).unwrap();
    let re_reg_index = Regex::new(REG_DEF_INDEX).unwrap();
    let re_reg_desc = Regex::new(REG_DESC).unwrap();
    let re_reg_bit_info = Regex::new(REG_BIT_INFO).unwrap();
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
            let mut buffer = vec![];
            let file_data = file_to_string(name);

            let mut state = State::FindReg;
            for line in file_data.lines() {
                match state {
                    State::FindReg => {
                        /* Normal register definitions */
                        if let Some(m) = re_reg.captures(line) {
                            let reg_name = &m[1];
                            let pname = &m[2];
                            let offset = &m[3].trim_start_matches("0x");
                            if reg_name.ends_with("(i)") {
                                // some indexed still get through, ignore them
                                continue;
                            }
                            if let Ok(addr) = u32::from_str_radix(offset, 16) {
                                let mut r = Register::default();
                                r.description = reg_name.to_string();
                                r.name = reg_name.to_string();
                                r.address = addr;
                                state = State::FindBitFieldInfo(pname.to_string(), r);
                            } else {
                                println!("Failed to parse register for {}: {}", reg_name, offset)
                            }
                        } /* else if let Some(m) = re_reg_index.captures(line) {
                              let reg_name = &m[1];
                              let pname = &m[2];
                              let offset = &m[3].trim_start_matches("0x");
                              if reg_name.ends_with("(i)") {
                                  // some indexed still get through, ignore them
                                  continue;
                              }
                              if let Ok(addr) = u32::from_str_radix(offset, 16) {
                                  let p = peripherals.get_mut(&pname.to_string()).unwrap();
                                  let mut r = Register::default();
                                  r.description = reg_name.to_string();
                                  r.name = reg_name.to_string();
                                  r.address = addr;
                                  p.registers.push(r);
                                  state = State::FindBitFieldInfo;
                              } else {
                                  println!("Failed to parse register for {}: {}", reg_name, offset)
                              }
                          } */
                    }
                    State::FindBitFieldInfo(ref mut pname, ref mut reg) => {
                        if let Some(m) = re_reg_bit_info.captures(line) {
                            let bf_name = &m[1];
                            let _access_type = &m[2]; // TODO
                            let bits = &mut m[3].split(':');
                            let _default_val = &m[4]; // TODO
                            let bits = match (bits.next(), bits.next()) {
                                (Some(h), Some(l)) => {
                                    Bits::Range(l.parse().unwrap()..=h.parse().unwrap())
                                }
                                (Some(b), None) => Bits::Single(b.parse().unwrap()),
                                _ => {
                                    println!("Failed to parse bitpos {}", &m[3]);
                                    continue;
                                }
                            };

                            let bf = BitField {
                                name: bf_name.to_string(),
                                bits,
                                reset_value: 0,
                                ..Default::default()
                            };
                            state = State::FindDescription(pname.clone(), reg.clone(), bf);
                        } else {
                            println!("Failed to match reg info");
                            state = State::FindReg;
                        }
                    }
                    State::FindDescription(ref mut pname, ref mut reg, ref mut bf) => {
                        buffer.push(line);
                        if let Some(m) = re_reg_desc.captures(buffer.join("").as_str()) {
                            bf.description = m[1].to_string();
                            buffer.clear();
                            reg.bit_fields.push(bf.clone()); // add the bit field to the reg
                            state = State::CheckEnd(pname.clone(), reg.clone());
                        }
                    }
                    State::CheckEnd(ref mut pname, ref mut reg) => {
                        if line.is_empty() {
                            println!("{} Adding {:#?}", pname, reg);
                            // were done with this register
                            if let Some(p) = peripherals.get_mut(&pname.to_string()) {
                                p.registers.push(reg.clone());
                            } else {
                                println!("No periphal called {}", pname.to_string());
                            }
                            state = State::FindReg;
                        } else if line.starts_with("/*") {
                            unimplemented!(); // TODO we need to process the current line, but changing the state will bin that line
                                              // weve found the next bit field in the reg
                            state = State::FindBitFieldInfo(pname.clone(), reg.clone());
                        } else {
                            // what do here?
                        }
                    }
                }
            }

            // for register in re_reg.captures_iter(file_data.as_str()) {
            //     let reg_name = &register[1];
            //     let pname = &register[2];
            //     let offset = &register[3].trim_start_matches("0x");
            //     if reg_name.ends_with("(i)") { // some indexed still get through, ignore them
            //         continue;
            //     }
            //     if let Ok(addr) = u32::from_str_radix(offset, 16) {
            //         let p = peripherals.get_mut(&pname.to_string()).unwrap();
            //         let mut r = Register::default();
            //         r.description = reg_name.to_string();
            //         r.name = reg_name.to_string();
            //         r.address = addr;
            //         p.registers.push(r);
            //     } else {
            //         println!("Failed to parse register for {}: {}", reg_name, offset)
            //     }
            // }

            // /* Indexed register definitions */
            // for register in re_reg_index.captures_iter(file_data.as_str()) {
            //     let reg_name = &register[1];
            //     let pname = &register[2];
            //     let offset = &register[3].trim_start_matches("0x");
            //     if let Ok(addr) = u32::from_str_radix(offset, 16) {
            //         if let Some(p) = peripherals.get_mut(&pname.to_string()) {
            //             let mut r = Register::default();
            //             r.description = reg_name.to_string();
            //             r.name = reg_name.to_string();
            //             r.address = addr;
            //             p.registers.push(r);
            //         } else {
            //             println!("No periphal called {}", pname.to_string());
            //         }
            //     } else {
            //         println!("Failed to parse register for {}: {}", reg_name, offset)
            //     }
            // }
        });

    // println!("{:#?}", peripherals);
}

fn file_to_string(fil: &str) -> String {
    let mut soc = File::open(fil).unwrap();
    let mut data = String::new();
    soc.read_to_string(&mut data).unwrap();
    data
}
