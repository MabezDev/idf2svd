use std::{collections::HashMap, fs::File, io::BufWriter, str::FromStr};

use regex::Regex;
use svd_parser::encode::Encode;

use crate::common::{
    build_svd, file_to_string, BitField, Bits, ChipType, Interrupt, Peripheral, Register, Type,
};

// Regexes to find all the peripheral addresses
const REG_BASE: &'static str = r"\#define[\s*]+DR_REG_(.*)_BASE[\s*]+0x([0-9a-fA-F]+)";
const REG_DEF: &'static str = r"\#define[\s*]+([^\s*]+)_REG[\s*]+\(DR_REG_(.*)_BASE \+ (.*)\)";
const REG_DEF_INDEX: &'static str =
    r"\#define[\s*]+([^\s*]+)_REG\(i\)[\s*]+\(REG_([0-9A-Za-z_]+)_BASE[\s*]*\(i\) \+ (.*?)\)";
const REG_BIT_INFO: &'static str = r"/\*[\s]+([0-9A-Za-z_]+)[\s]+:[\s]+([0-9A-Za-z_/]+)[\s]+;bitpos:\[(.*)\][\s];default:[\s]+(.*)[\s];[\s]\*/";
const REG_DESC: &'static str = r"\*description:\s(.*[\n|\r|\r\n]?.*)\*/";
const INTERRUPTS: &'static str =
    r"\#define[\s]ETS_([0-9A-Za-z_/]+)_SOURCE[\s]+([0-9]+)/\*\*<\s([0-9A-Za-z_/\s,]+)\*/";

enum State {
    FindReg,
    FindBitFieldInfo(String, Register),
    FindDescription(String, Register, BitField),
    CheckEnd(String, Register),
}

fn parse_idf(chip: &ChipType) -> HashMap<String, Peripheral> {
    let mut peripherals = HashMap::new();
    let mut interrupts = vec![];

    let mut invalid_bit_fields = vec![];
    let mut invalid_files = vec![];
    let mut invalid_peripherals = vec![];
    let mut invalid_registers = vec![];

    let re_base = Regex::new(REG_BASE).unwrap();
    let re_reg = Regex::new(REG_DEF).unwrap();
    let re_reg_index = Regex::new(REG_DEF_INDEX).unwrap();
    let re_reg_bit_info = Regex::new(REG_BIT_INFO).unwrap();
    let re_reg_desc = Regex::new(REG_DESC).unwrap();
    let re_interrupts = Regex::new(INTERRUPTS).unwrap();

    let soc_base_path = format!(
        "esp-idf/components/soc/{}/include/soc",
        chip.to_string().to_lowercase()
    );

    let filename = format!("{}/{}", soc_base_path, "soc.h");
    let soc_h = file_to_string(&filename);

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
       These are indexed, we seed these as they cannot be derived from the docs
       These blocks are identical, so we need to do some post processing to properly index
       and offset these
    */
    peripherals.insert("I2C".to_string(), Peripheral::default());
    peripherals.insert("SPI".to_string(), Peripheral::default());
    peripherals.insert("TIMG".to_string(), Peripheral::default());
    peripherals.insert("MCPWM".to_string(), Peripheral::default());
    peripherals.insert("UHCI".to_string(), Peripheral::default());

    if *chip == ChipType::ESP32C3 {
        peripherals.insert("I2S".to_string(), Peripheral::default());
        peripherals.insert("SPI_MEM".to_string(), Peripheral::default());
        peripherals.insert("GPIO_SD".to_string(), Peripheral::default());
        peripherals.insert("INTERRUPT_CORE0".to_string(), Peripheral::default());
    }

    /* Peripheral base addresses */
    for captures in re_base.captures_iter(soc_h.as_str()) {
        let peripheral = &captures[1];
        let address = &captures[2];

        let mut p = Peripheral::default();
        p.address = u32::from_str_radix(address, 16).unwrap();
        p.description = peripheral.to_string();

        peripherals.insert(peripheral.to_string(), p);
    }

    std::fs::read_dir(soc_base_path)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|f| f.path().to_str().unwrap().ends_with("_reg.h"))
        .for_each(|f| {
            let name = f.path();
            let name = name.to_str().unwrap();

            let mut buffer = vec![];
            let file_data = file_to_string(name);

            let mut something_found = false;
            let mut state = State::FindReg;
            for (i, line) in file_data.lines().enumerate() {
                loop {
                    match state {
                        State::FindReg => {
                            let captures = if re_reg.is_match(line) {
                                re_reg.captures(line)
                            } else if re_reg_index.is_match(line) {
                                re_reg_index.captures(line)
                            } else {
                                None
                            };

                            if let Some(m) = captures {
                                let reg_name = &m[1];
                                let pname = &m[2];
                                let offset = &m[3].trim_start_matches("0x");

                                if let Ok(addr) = u32::from_str_radix(offset, 16) {
                                    let mut r = Register::default();
                                    r.name = reg_name.to_string();
                                    r.description = reg_name.to_string();
                                    r.address = addr;
                                    state = State::FindBitFieldInfo(pname.to_string(), r);
                                } else {
                                    invalid_registers.push(reg_name.to_string());
                                }
                            }
                            break; // next line
                        }
                        State::FindBitFieldInfo(ref mut pname, ref mut reg) => {
                            something_found = true;
                            if let Some(m) = re_reg_bit_info.captures(line) {
                                let bf_name = &m[1];
                                let access_type = &m[2]; // TODO
                                let bits = &mut m[3].split(':');
                                let _default_val = &m[4]; // TODO
                                let bits = match (bits.next(), bits.next()) {
                                    (Some(h), Some(l)) => {
                                        Bits::Range(l.parse().unwrap()..=h.parse().unwrap())
                                    }
                                    (Some(b), None) => Bits::Single(b.parse().unwrap()),
                                    _ => {
                                        // println!("Failed to parse bitpos {}", &m[3]);
                                        invalid_bit_fields
                                            .push((bf_name.to_string(), m[3].to_string()));
                                        continue;
                                    }
                                };

                                let bf = BitField {
                                    name: bf_name.to_string(),
                                    bits,
                                    type_: Type::from_str(access_type).unwrap_or_else(|s| {
                                        println!("{}", s);
                                        Type::default()
                                    }),
                                    reset_value: 0,
                                    ..Default::default()
                                };
                                state = State::FindDescription(pname.clone(), reg.clone(), bf);
                            } else {
                                println!("Failed to match reg info at {}:{} ('{}')", name, i, line);
                                state = State::FindReg;
                            }
                            break; // next line
                        }
                        State::FindDescription(ref mut pname, ref mut reg, ref mut bf) => {
                            buffer.push(line);
                            if let Some(_m) = re_reg_desc.captures(buffer.join("").as_str()) {
                                buffer.clear();
                                reg.bit_fields.push(bf.clone()); // add the bit field to the reg
                                state = State::CheckEnd(pname.clone(), reg.clone());
                            }
                            break; // next line
                        }
                        State::CheckEnd(ref mut pname, ref mut reg) => {
                            if line.is_empty() {
                                // we're done with this register
                                if let Some(p) = peripherals.get_mut(&pname.to_string()) {
                                    p.registers.push(reg.clone());
                                } else {
                                    // TODO indexed peripherals wont come up here
                                    println!("No peripheral called {}", pname.to_string());
                                    invalid_peripherals.push(pname.to_string());
                                }
                                state = State::FindReg;
                                break; // next line
                            } else if re_reg_bit_info.is_match(line) {
                                // we've found the next bit field in the reg
                                state = State::FindBitFieldInfo(pname.clone(), reg.clone());
                            } else {
                                break; // next line
                            }
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

    if invalid_bit_fields.len() > 0 {
        println!(
            "The following bit_fields failed to parse {:?}",
            invalid_bit_fields
        );
    }

    // println!("Interrupt information: {:#?}", interrupts);

    peripherals
}

pub fn create_svd(chip: ChipType) {
    let peripherals = parse_idf(&chip);
    let filename = format!("{}.svd", chip.to_string().to_lowercase());
    let svd = build_svd(chip, peripherals).unwrap();

    let f = BufWriter::new(File::create(filename).unwrap());
    svd.encode().unwrap().write(f).unwrap();
}
