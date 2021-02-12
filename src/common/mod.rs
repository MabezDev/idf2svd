use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::ops::RangeInclusive;
use std::str::FromStr;
use std::string::ToString;

use svd_parser::{
    addressblock::AddressBlock, bitrange::BitRangeType, cpu::CpuBuilder, device::DeviceBuilder,
    endian::Endian, fieldinfo::FieldInfoBuilder, peripheral::PeripheralBuilder,
    registerinfo::RegisterInfoBuilder, Access, BitRange, Device as SvdDevice, Field,
    Register as SvdRegister, RegisterCluster,
};

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ChipType {
    ESP32,
    ESP32C3,
    ESP8266,
}

impl ChipType {
    pub fn detailed_name(&self) -> String {
        match self {
            ChipType::ESP32 => "Xtensa LX6".to_owned(),
            ChipType::ESP32C3 => "RISC-V RV32IMC single-core".to_owned(),
            ChipType::ESP8266 => "Xtensa LX106".to_owned(),
        }
    }
}

impl ToString for ChipType {
    fn to_string(&self) -> String {
        match self {
            ChipType::ESP32 => "ESP32".to_owned(),
            ChipType::ESP32C3 => "ESP32C3".to_owned(),
            ChipType::ESP8266 => "ESP8266".to_owned(),
        }
    }
}

impl FromStr for ChipType {
    type Err = String;

    fn from_str(s: &str) -> Result<ChipType, Self::Err> {
        Ok(match s {
            "ESP32" => ChipType::ESP32,
            "ESP32C3" => ChipType::ESP32C3,
            "ESP8266" => ChipType::ESP8266,
            _ => return Err(String::from("Invalid chip: ") + &String::from(s)),
        })
    }
}

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
            "R/WTC/SS" => Type::ReadWrite,
            "R/W/WTC/SS" => Type::ReadWrite,
            "R/SS/WTC" => Type::ReadWrite,
            "R/W/SC" => Type::ReadWrite,
            "R/W/SS" => Type::ReadWrite,
            "R/W/SS/SC" => Type::ReadWrite,
            "R/W/WTC" => Type::ReadWrite,
            "WOD" => Type::WriteOnly,
            "WT" => Type::WriteOnly,
            _ => return Err(String::from("Invalid BitField type: ") + &String::from(s)),
        })
    }
}

pub fn file_to_string(file: &str) -> String {
    let mut soc = File::open(file).unwrap();
    let mut data = String::new();
    soc.read_to_string(&mut data).unwrap();

    data
}

pub fn build_svd(
    chip: ChipType,
    peripherals: HashMap<String, Peripheral>,
) -> Result<SvdDevice, ()> {
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
                    .access(Some(field.type_.into()))
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
            .registers(if registers.is_empty() {
                None
            } else {
                Some(registers)
            })
            .address_block(Some(AddressBlock {
                offset: 0x0,
                size: block_size, // TODO what about derived peripherals?
                usage: "registers".to_string(),
            }))
            .build()
            .unwrap();

        svd_peripherals.push(out);
    }

    svd_peripherals.sort_by(|a, b| a.name.cmp(&b.name));

    println!("Len {}", svd_peripherals.len());

    let cpu = CpuBuilder::default()
        .name(chip.detailed_name())
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
        .name(chip.to_string())
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
