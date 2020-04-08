use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufWriter;
use std::ops::RangeInclusive;
use std::str::FromStr;

use regex::Regex;
use svd_parser::{
    addressblock::AddressBlock, bitrange::BitRangeType, cpu::CpuBuilder, device::DeviceBuilder,
    encode::Encode, endian::Endian, fieldinfo::FieldInfoBuilder, peripheral::PeripheralBuilder,
    registerinfo::RegisterInfoBuilder, Access, BitRange, Device as SvdDevice, Field,
    Register as SvdRegister, RegisterCluster,
};

const SOC_BASE_PATH: &'static str = "esp-idf/components/soc/esp32/include/soc/";

// Regexes to find all the peripheral addresses
const REG_BASE: &'static str = r"\#define[\s*]+DR_REG_(.*)_BASE[\s*]+0x([0-9a-fA-F]+)";
const REG_DEF: &'static str = r"\#define[\s*]+([^\s*]+)_REG[\s*]+\(DR_REG_(.*)_BASE \+ (.*)\)";
const REG_DEF_INDEX: &'static str =
    r"\#define[\s*]+([^\s*]+)_REG\(i\)[\s*]+\(REG_([0-9A-Za-z_]+)_BASE[\s*]*\(i\) \+ (.*?)\)";
const REG_BIT_INFO: &'static str = r"/\*[\s]+([0-9A-Za-z_]+)[\s]+:[\s]+([0-9A-Za-z_/]+)[\s]+;bitpos:\[(.*)\][\s];default:[\s]+(.*)[\s];[\s]\*/";
const REG_DESC: &'static str = r"\*description:\s(.*[\n|\r|\r\n]?.*)\*/";
const INTERRUPTS: &'static str =
    r"\#define[\s]ETS_([0-9A-Za-z_/]+)_SOURCE[\s]+([0-9]+)/\*\*<\s([0-9A-Za-z_/\s,]+)\*/";

#[derive(Debug, Default, Clone)]
pub struct Peripheral {
    pub description: String,
    pub address: u32,
    pub registers: Vec<Register>,
}

#[derive(Clone, Debug, Default)]
pub struct Interrupt {
    pub name: String,
    pub description: Option<String>,
    pub value: u32,
}

#[derive(Debug, Default, Clone)]
pub struct Register {
    /// Register Name
    pub name: String,
    /// Relative Address
    pub address: u32,
    /// Width
    pub width: u8,
    /// Description
    pub description: String,
    /// Reset Value
    pub reset_value: u64,
    /// Detailed description
    pub detailed_description: Option<String>,
    pub bit_fields: Vec<BitField>,
}

#[derive(Debug, Default, Clone)]
pub struct BitField {
    /// Field Name
    pub name: String,
    /// Bits
    pub bits: Bits,
    /// Type
    pub type_: Type,
    /// Reset Value
    pub reset_value: u32,
    /// Description
    pub description: String,
}

#[derive(Debug, Clone)]
pub enum Bits {
    Single(u8),
    Range(RangeInclusive<u8>),
}

impl Default for Bits {
    fn default() -> Self {
        Bits::Single(0)
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Type {
    // ReadAsZero,
    ReadOnly,
    ReadWrite,
    WriteOnly,
    // ReadWriteSetOnly,
    // ReadableClearOnRead,
    // ReadableClearOnWrite,
    // WriteAsZero,
    // WriteToClear,
}

impl From<Type> for Access {
    fn from(t: Type) -> Self {
        match t {
            Type::ReadOnly => Access::ReadOnly,
            Type::ReadWrite => Access::ReadWrite,
            Type::WriteOnly => Access::WriteOnly,
        }
    }
}

impl Default for Type {
    fn default() -> Type {
        Type::ReadWrite
    }
}

impl FromStr for Type {
    type Err = String;

    fn from_str(s: &str) -> Result<Type, Self::Err> {
        Ok(match s {
            "RO" | "R/O" => Type::ReadOnly,
            "RW" | "R/W" => Type::ReadWrite,
            "WO" | "W/O" => Type::WriteOnly,
            _ => return Err(String::from("Invalid BitField type: ") + &String::from(s)),
        })
    }
}

enum State {
    FindReg,
    FindBitFieldInfo(String, Register),
    FindDescription(String, Register, BitField),
    CheckEnd(String, Register),
}

fn parse_idf() -> HashMap<String, Peripheral> {
    let mut peripherals = HashMap::new();
    let mut invalid_peripherals = vec![];
    let mut invalid_files = vec![];
    let mut invalid_registers = vec![];
    let mut invalid_bit_fields = vec![];

    let mut interrupts = vec![];

    let filname = SOC_BASE_PATH.to_owned() + "soc.h";
    let re_base = Regex::new(REG_BASE).unwrap();
    let re_reg = Regex::new(REG_DEF).unwrap();
    let re_reg_index = Regex::new(REG_DEF_INDEX).unwrap();
    let re_reg_desc = Regex::new(REG_DESC).unwrap();
    let re_reg_bit_info = Regex::new(REG_BIT_INFO).unwrap();
    let re_interrupts = Regex::new(INTERRUPTS).unwrap();

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
    peripherals.insert("I2C".to_string(), Peripheral::default());
    peripherals.insert("SPI".to_string(), Peripheral::default());
    peripherals.insert("TIMG".to_string(), Peripheral::default());
    peripherals.insert("MCPWM".to_string(), Peripheral::default());
    peripherals.insert("UHCI".to_string(), Peripheral::default());

    /* Peripheral base addresses */
    for captures in re_base.captures_iter(soc_h.as_str()) {
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
            // println!("Searching {}", name);
            let mut something_found = false;
            let mut state = State::FindReg;
            for (i, line) in file_data.lines().enumerate() {
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
                                    state = State::FindBitFieldInfo(pname.to_string(), r);
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
                                println!("Failed to match reg info at {}:{}", name, i);
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
                                // println!("{} Adding {:#?}", pname, reg);
                                // were done with this register
                                if let Some(p) = peripherals.get_mut(&pname.to_string()) {
                                    p.registers.push(reg.clone());
                                } else {
                                    // TODO indexed peripherals wont come up here
                                    // println!("No periphal called {}", pname.to_string());
                                    invalid_peripherals.push(pname.to_string());
                                }
                                state = State::FindReg;
                                break; // next line
                            } else if re_reg_bit_info.is_match(line) {
                                // weve found the next bit field in the reg
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

fn file_to_string(fil: &str) -> String {
    let mut soc = File::open(fil).unwrap();
    let mut data = String::new();
    soc.read_to_string(&mut data).unwrap();
    data
}

fn build_svd(peripherals: HashMap<String, Peripheral>) -> Result<SvdDevice, ()> {
    let mut svd_peripherals = vec![];

    for (name, p) in peripherals {
        let mut registers = vec![];
        for r in p.registers {
            let mut fields = vec![];
            for field in &r.bit_fields {
                let description = if field.description.trim().is_empty() {
                    None
                } else {
                    Some(field.description.clone())
                };

                let bit_range = match &field.bits {
                    Bits::Single(bit) => BitRange {
                        offset: u32::from(*bit),
                        width: 1,
                        range_type: BitRangeType::OffsetWidth,
                    },
                    Bits::Range(r) => BitRange {
                        offset: u32::from(*r.start()),
                        width: u32::from(r.end() - r.start() + 1),
                        range_type: BitRangeType::OffsetWidth,
                    },
                };

                let field_out = FieldInfoBuilder::default()
                    .name(field.name.clone())
                    .description(description)
                    .bit_range(bit_range)
                    .build()
                    .unwrap();
                fields.push(Field::Single(field_out));
            }

            let info = RegisterInfoBuilder::default()
                .name(r.name.clone())
                .description(Some(r.description.clone()))
                .address_offset(r.address)
                .size(Some(32))
                .reset_value(Some(r.reset_value as u32))
                .fields(Some(fields))
                .build()
                .unwrap();

            registers.push(RegisterCluster::Register(SvdRegister::Single(info)));
        }
        let block_size = registers.iter().fold(0, |sum, reg| {
            sum + match reg {
                RegisterCluster::Register(r) => r.size.unwrap(),
                _ => unimplemented!(),
            }
        });
        let out = PeripheralBuilder::default()
            .name(name.to_owned())
            .base_address(p.address)
            .registers(Some(registers))
            .address_block(Some(AddressBlock {
                offset: 0x0,
                size: block_size, // TODO what about derived peripherals?
                usage: "registers".to_string(),
            }))
            .build()
            .unwrap();

        svd_peripherals.push(out);
    }
    println!("Len {}", svd_peripherals.len());

    let cpu = CpuBuilder::default()
        .name("Xtensa LX6".to_string())
        .revision("1".to_string())
        .endian(Endian::Little)
        .mpu_present(false)
        .fpu_present(true)
        // according to https://docs.espressif.com/projects/esp-idf/en/latest/api-reference/system/intr_alloc.html#macros
        // 7 levels so 3 bits? //TODO verify
        .nvic_priority_bits(3)
        .has_vendor_systick(false)
        .build()
        .unwrap();

    let device = DeviceBuilder::default()
        .name("Espressif".to_string())
        .version(Some("1.0".to_string()))
        .schema_version(Some("1.0".to_string()))
        // broken see: https://github.com/rust-embedded/svd/pull/104
        // .description(Some("ESP32".to_string()))
        // .address_unit_bits(Some(8))
        .width(Some(32))
        .cpu(Some(cpu))
        .peripherals(svd_peripherals)
        .build()
        .unwrap();

    Ok(device)
}

pub fn create_svd() {
    let peripherals = parse_idf();
    let svd = build_svd(peripherals).unwrap();

    let f = BufWriter::new(File::create("esp32.svd").unwrap());
    svd.encode().unwrap().write(f).unwrap();
}