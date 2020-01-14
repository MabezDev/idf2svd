pub const SOC_BASE_PATH: &'static str = "esp-idf/components/soc/esp32/include/soc/";

use header2svd::{parse_idf, Bits, Peripheral};

use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;
use svd_parser::{
    addressblock::AddressBlock, bitrange::BitRangeType, cpu::CpuBuilder, device::DeviceBuilder,
    encode::Encode, endian::Endian, fieldinfo::FieldInfoBuilder, peripheral::PeripheralBuilder,
    registerinfo::RegisterInfoBuilder, BitRange, Device as SvdDevice, Field,
    Register as SvdRegister, RegisterCluster,
};

fn main() {
    let peripherals = parse_idf(SOC_BASE_PATH);

    let svd = create_svd(peripherals).unwrap();

    let f = BufWriter::new(File::create("esp32.svd").unwrap());
    svd.encode().unwrap().write(f).unwrap();
}

fn create_svd(peripherals: HashMap<String, Peripheral>) -> Result<SvdDevice, ()> {
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
