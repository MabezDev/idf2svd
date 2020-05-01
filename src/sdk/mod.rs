use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;

use regex::Regex;
use svd_parser::encode::Encode;

use crate::common::{
    build_svd, file_to_string, BitField, Bits, Interrupt, Peripheral, Register, Type,
};

mod doc_input;
mod doc_parse;

pub use doc_parse::parse_doc;

pub const SOC_BASE_PATH: &'static str = "ESP8266_RTOS_SDK/components/esp8266/include/esp8266/";

// make the header a bit more easy to handle
const REPLACEMENTS: &'static [(&'static str, &'static str)] = &[
    ("PERIPHS_IO_MUX ", "PERIPHS_IO_MUX_BASE "),
    ("RTC_STORE0", "RTC_STORE0_REG"),
    ("RTC_STATE1", "RTC_STATE1_REG"),
    ("RTC_STATE2", "RTC_STATE2_REG"),
    ("(0x60000000 + (i)*0xf00)", "0x60000000"), // uart base address
];
const REPLACEMENTS_REGEX: &'static [(&'static str, &'static str)] = &[
    (r"(I2S[^\s]+)[\s]+(\(REG_I2S_BASE \+ )", "${1}_REG $2"),
    (r"(SLC_[^\s]+)[\s]+(\(REG_SLC_BASE \+ )", "${1}_REG $2"),
];

// Regexes to find all the peripheral addresses
pub const REG_BASE: &'static str =
    r"\#define[\s*]+(?:DR_REG|REG|PERIPHS)_(.*)_BASE(?:_?A?DDR)?[\s*]+\(?0x([0-9a-fA-F]+)\)?";
pub const REG_DEF: &'static str = r"\#define[\s*]+(?:PERIPHS_)?([^\s*]+)_(?:REG|ADDRESS|U|ADDR)[\s*]+\((?:DR_REG|REG|PERIPHS)_(.*)_BASE(?:_?A?DDR)? \+ (.*)\)";
pub const REG_DEF_OFFSET: &'static str =
    r"\#define[\s*]+(?:PERIPHS_)?([^\s*]+)_(?:ADDRESS|U|ADDR)[\s*]+(?:0x)?([0-9a-fA-F]+)";
pub const REG_DEF_INDEX: &'static str = r"\#define[\s*]+(?:PERIPHS_)?([^\s*]+)_(?:REG|ADDRESS|U|ADDR)\(i\)[\s*]+\((?:DR_REG|REG|PERIPHS)_([0-9A-Za-z_]+)_BASE(?:_?A?DDR)?[\s*]*\(i\) \+ (.*?)\)";
pub const REG_DEFINE_MASK: &'static str = r"\#define[\s*]+(?:PERIPHS_)?([^\s*]+)[\s*]+\(?(0x[0-9a-fA-F]+|[0-9]+|\(?BIT\(?[0-9]+\)?)\)?\)?";
pub const REG_DEFINE_SHIFT: &'static str =
    r"\#define[\s*]+(?:PERIPHS_)?([^\s*]+)_(?:S|s)[\s*]+\(?(0x[0-9a-fA-F]+|[0-9]+)\)?";
pub const REG_DEFINE_SKIP: &'static str =
    r"\#define[\s*]+(?:PERIPHS_)?([^\s*]+)_(?:M|V)[\s*]+(\(|0x)";
pub const SINGLE_BIT: &'static str = r"BIT\(?([0-9]+)\)?";
pub const INTERRUPTS: &'static str =
    r"\#define[\s]ETS_([0-9A-Za-z_/]+)_SOURCE[\s]+([0-9]+)/\*\*<\s([0-9A-Za-z_/\s,]+)\*/";
pub const REG_IFDEF: &'static str = r"#ifn?def.*";
pub const REG_ENDIF: &'static str = r"#endif";

enum State {
    FindReg,
    FindBitFieldMask(String, Register),
    FindBitFieldShift(String, Register, u32),
    FindBitFieldSkipShift(String, Register),
    AssumeFullRegister(String, Register),
    CheckEnd(String, Register),
    End(String, Register),
}

fn add_base_addr(header: &str, peripherals: &mut HashMap<String, Peripheral>) {
    let re_base = Regex::new(REG_BASE).unwrap();

    // Peripheral base addresses
    for captures in re_base.captures_iter(header) {
        let peripheral = &captures[1];
        let address = &captures[2];

        let mut p = Peripheral::default();
        p.address = u32::from_str_radix(address, 16).unwrap();
        p.description = peripheral.to_string();

        if !peripherals.contains_key(peripheral) {
            peripherals.insert(peripheral.to_string(), p);
        }
    }
}

fn parse_sdk() -> HashMap<String, Peripheral> {
    let mut peripherals = HashMap::new();
    let mut invalid_peripherals = vec![];
    let mut invalid_files = vec![];
    let mut invalid_registers = vec![];
    // let mut invalid_bit_fields = vec![];

    let mut interrupts = vec![];

    let filname = SOC_BASE_PATH.to_owned() + "eagle_soc.h";
    let re_reg = Regex::new(REG_DEF).unwrap();
    let re_reg_index = Regex::new(REG_DEF_INDEX).unwrap();
    let re_reg_offset = Regex::new(REG_DEF_OFFSET).unwrap();
    let re_reg_define = Regex::new(REG_DEFINE_MASK).unwrap();
    let re_reg_define_shift = Regex::new(REG_DEFINE_SHIFT).unwrap();
    let re_interrupts = Regex::new(INTERRUPTS).unwrap();
    let re_single_bit = Regex::new(SINGLE_BIT).unwrap();
    let re_reg_skip = Regex::new(REG_DEFINE_SKIP).unwrap();
    let re_ifdef = Regex::new(REG_IFDEF).unwrap();
    let re_endif = Regex::new(REG_ENDIF).unwrap();

    let soc_h = file_to_string(&filname);

    for captures in re_interrupts.captures_iter(soc_h.as_str()) {
        let name = &captures[1];
        let index = &captures[2];
        let desc = &captures[3];
        let intr = Interrupt {
            name: name.to_string(),
            description: Some(desc.to_string()),
            value: index.parse().unwrap(),
        };
        interrupts.push(intr);
        // println!("{:#?}", intr);
    }

    /*
       Theses are indexed, we seed these as they cannot be derived from the docs
       These blocks are identical, so we need to do some post processing to properly index
       and offset these
    */
    // peripherals.insert("I2C".to_string(), Peripheral::default());
    // peripherals.insert("SPI".to_string(), Peripheral::default());
    // peripherals.insert("TIMG".to_string(), Peripheral::default());
    // peripherals.insert("MCPWM".to_string(), Peripheral::default());
    // peripherals.insert("UHCI".to_string(), Peripheral::default());

    add_base_addr(&soc_h, &mut peripherals);

    std::fs::read_dir(SOC_BASE_PATH)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|f| {
            f.path().to_str().unwrap().ends_with("_register.h")
                || f.file_name().to_str().unwrap() == "eagle_soc.h"
        })
        .for_each(|f| {
            let name = f.path();
            let name = name.to_str().unwrap();
            // let mut buffer = vec![];
            let mut file_data = file_to_string(name);
            for (search, replace) in REPLACEMENTS {
                file_data = file_data.replace(search, replace);
            }

            for (search, replace) in REPLACEMENTS_REGEX {
                let re = Regex::new(search).unwrap();
                file_data = re.replace_all(&file_data, *replace).to_string();
            }

            add_base_addr(&file_data, &mut peripherals);

            // println!("Searching {}", name);
            let mut something_found = false;
            let mut state = State::FindReg;
            for (i, line) in file_data.lines().enumerate() {
                if re_ifdef.is_match(line) {
                    continue;
                } else if re_endif.is_match(line) {
                    continue;
                }

                loop {
                    match state {
                        State::FindReg => {
                            /* Normal register definitions */
                            if let Some(m) = re_reg.captures(line) {
                                let reg_name = &m[1];
                                let pname = &m[2];
                                let offset = &m[3].trim_start_matches("0x");
                                if reg_name.ends_with("(i)") {
                                    invalid_registers.push(reg_name.to_string());
                                    // some indexed still get through, ignore them
                                    break;
                                }
                                if let Ok(addr) = u32::from_str_radix(offset, 16) {
                                    let mut r = Register::default();
                                    r.description = reg_name.to_string();
                                    r.name = reg_name.to_string();
                                    r.address = addr;
                                    state = State::FindBitFieldMask(pname.to_string(), r);
                                } else {
                                    invalid_registers.push(reg_name.to_string());
                                }
                            } else if let Some(m) = re_reg_index.captures(line) {
                                let reg_name = &m[1];
                                let pname = &m[2];
                                let offset = &m[3].trim_start_matches("0x");

                                if let Ok(addr) = u32::from_str_radix(offset, 16) {
                                    let mut r = Register::default();
                                    r.name = reg_name.to_string();
                                    r.description = reg_name.to_string();
                                    r.address = addr;
                                    state = State::FindBitFieldMask(pname.to_string(), r);
                                } else {
                                    invalid_registers.push(reg_name.to_string());
                                }
                            } else if let Some(m) = re_reg_offset.captures(line) {
                                let reg_name = &m[1];
                                let offset = &m[2];
                                let pname = reg_name.split('_').next().unwrap();

                                if let Ok(addr) = u32::from_str_radix(offset, 16) {
                                    let mut r = Register::default();
                                    r.name = reg_name.to_string();
                                    r.description = reg_name.to_string();
                                    r.address = addr;
                                    state = State::FindBitFieldMask(pname.to_string(), r);
                                } else {
                                    invalid_registers.push(reg_name.to_string());
                                }
                            }
                            break; // next line
                        }
                        State::AssumeFullRegister(ref mut pname, ref mut reg) => {
                            something_found = true;
                            // assume full 32bit wide field
                            let bitfield = BitField {
                                name: "Register".to_string(),
                                bits: Bits::Range(0..=31),
                                ..Default::default()
                            };
                            reg.bit_fields.push(bitfield);

                            if let Some(p) = peripherals.get_mut(&pname.to_string()) {
                                p.registers.push(reg.clone());
                            } else {
                                invalid_peripherals.push(pname.to_string());
                            }
                            state = State::FindReg;
                        }
                        State::FindBitFieldMask(ref mut pname, ref mut reg) => {
                            if re_reg_skip.is_match(line) {
                                break;
                            }

                            if re_reg_offset.is_match(line) {
                                state = State::AssumeFullRegister(pname.clone(), reg.clone());
                                continue;
                            }
                            if let Some(m) = re_reg_define.captures(line) {
                                something_found = true;
                                let define_name = &m[1];
                                let value = &m[2].trim_start_matches("0x");

                                if let Some(m) = re_single_bit.captures(value) {
                                    if let Ok(mask_bit) = u8::from_str_radix(&m[1], 10) {
                                        let bitfield = BitField {
                                            name: define_name.to_string(),
                                            bits: Bits::Single(mask_bit),
                                            ..Default::default()
                                        };
                                        reg.bit_fields.push(bitfield);
                                        state = State::FindBitFieldSkipShift(
                                            pname.clone(),
                                            reg.clone(),
                                        );
                                        break;
                                    } else {
                                        println!(
                                            "Failed to single bit match reg mask at {}:{}",
                                            name, i
                                        );
                                        state = State::FindReg;
                                    }
                                } else if let Ok(mask) = u32::from_str_radix(value, 16) {
                                    state =
                                        State::FindBitFieldShift(pname.clone(), reg.clone(), mask);
                                }
                            } else {
                                if reg.bit_fields.is_empty() {
                                    state = State::AssumeFullRegister(pname.clone(), reg.clone());
                                    continue;
                                } else {
                                    println!("Failed to match reg mask at {}:{}", name, i);
                                    state = State::End(pname.clone(), reg.clone());
                                }
                            }
                            break; // next line
                        }
                        State::FindBitFieldShift(ref mut pname, ref mut reg, ref mut mask) => {
                            if re_reg_skip.is_match(line) {
                                break;
                            }
                            if let Some(m) = re_reg_define_shift.captures(line) {
                                let define_name = &m[1];
                                let value = &m[2];

                                if let Ok(shift) = u8::from_str_radix(value, 10) {
                                    let bitfield = BitField {
                                        name: define_name.to_string(),
                                        bits: match mask.count_ones() {
                                            1 => Bits::Single(shift),
                                            bits => Bits::Range(shift..=shift + (bits - 1) as u8),
                                        },
                                        ..Default::default()
                                    };
                                    reg.bit_fields.push(bitfield);
                                    state = State::CheckEnd(pname.clone(), reg.clone())
                                }
                            } else {
                                if reg.bit_fields.is_empty() {
                                    state = State::AssumeFullRegister(pname.clone(), reg.clone());
                                    continue;
                                } else {
                                    println!(
                                        "Failed to match reg shift at {}:{} ('{}')",
                                        name, i, line
                                    );
                                    state = State::End(pname.clone(), reg.clone());
                                }
                            }
                            break; // next line
                        }
                        State::FindBitFieldSkipShift(ref mut pname, ref mut reg) => {
                            state = State::CheckEnd(pname.clone(), reg.clone());
                            if re_reg_define_shift.is_match(line) {
                                break;
                            }
                        }
                        State::CheckEnd(ref mut pname, ref mut reg) => {
                            if line.is_empty() {
                                state = State::End(pname.clone(), reg.clone());
                                break;
                            } else if re_reg_define.is_match(line) {
                                // we've found the next bit field in the reg
                                state = State::FindBitFieldMask(pname.clone(), reg.clone());
                            } else {
                                break; // next line
                            }
                        }
                        State::End(ref mut pname, ref mut reg) => {
                            if let Some(p) = peripherals.get_mut(&pname.to_string()) {
                                p.registers.push(reg.clone());
                            } else {
                                // TODO indexed peripherals wont come up here
                                // println!("No periphal called {}", pname.to_string());
                                invalid_peripherals.push(pname.to_string());
                            }
                            state = State::FindReg;
                        }
                    }
                }
            }

            // log if nothing was parsed in this file
            if !something_found {
                invalid_files.push(String::from(name))
            }
        });

    println!("Parsed idf for peripherals information.");

    if invalid_files.len() > 0 {
        println!(
            "The following files contained no parsable information {:?}",
            invalid_files
        );
    }

    if invalid_peripherals.len() > 0 {
        println!(
            "The following peripherals failed to parse {:?}",
            invalid_peripherals
        );
    }

    if invalid_registers.len() > 0 {
        println!(
            "The following registers failed to parse {:?}",
            invalid_registers
        );
    }

    // if invalid_bit_fields.len() > 0 {
    //     println!(
    //         "The following bit_fields failed to parse {:?}",
    //         invalid_bit_fields
    //     );
    // }

    // println!("Interrupt information: {:#?}", interrupts);

    peripherals
}

pub fn create_svd() {
    let mut peripherals = parse_sdk();

    // where available, the docs provide more detailed info
    peripherals
        .iter_mut()
        .for_each(|(name, peripheral)| match name.as_str() {
            "TIMER" => {
                let doc_peripheral = parse_doc("build/timer.json");
                peripheral.registers = doc_peripheral.registers;
            }
            "GPIO" => {
                let doc_peripheral = parse_doc("build/gpio.json");
                peripheral.registers = doc_peripheral.registers;
            }
            _ => {}
        });

    let mut uart_peripheral_0 = parse_doc("build/uart.json");
    let mut uart_peripheral_1 = uart_peripheral_0.clone();
    uart_peripheral_0.address = 0x60000000;
    uart_peripheral_1.address = 0x60000f00;
    peripherals.insert("UART0".to_string(), uart_peripheral_0);
    peripherals.insert("UART1".to_string(), uart_peripheral_1);

    let mut spi = parse_doc("build/spi.json");
    spi.address = 0x60000200;
    for i in 0..16 {
        spi.registers.push(Register {
            name: format!("SPI_W{}", i),
            address: 0x40 + (i * 32),
            width: 32,
            description: format!("the data inside the buffer of the SPI module, byte {}", i),
            reset_value: 0,
            bit_fields: vec![BitField {
                name: format!("spi_w{}", i),
                bits: Bits::Range(0..=31),
                type_: Type::ReadWrite,
                reset_value: 0,
                description: format!("the data inside the buffer of the SPI module, byte {}", i),
            }],
            detailed_description: None,
        })
    }
    peripherals.insert("SPI".to_string(), spi);

    let cpu_name = String::from("Xtensa LX106");
    let svd = build_svd(cpu_name, peripherals).unwrap();

    let f = BufWriter::new(File::create("esp8266.svd").unwrap());
    svd.encode().unwrap().write(f).unwrap();
}
