pub const SOC_BASE_PATH: &'static str = "esp-idf/components/soc/esp32/include/soc/";

use header2svd::{parse_idf, Bits, Peripheral};

use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;
use svd_parser::{
bitrange::BitRangeType, encode::Encode, fieldinfo::FieldInfoBuilder,
    peripheral::PeripheralBuilder, registerinfo::RegisterInfoBuilder, BitRange, Field,
    Register as SvdRegister, RegisterCluster,
};
use xmltree::Element;

fn main() {
    let peripherals = parse_idf(SOC_BASE_PATH);

    let svd = create_svd(peripherals).unwrap();

    let f = BufWriter::new(File::create("esp32.svd").unwrap());
    svd.write(f).unwrap();
}

fn create_svd(peripherals: HashMap<String, Peripheral>) -> Result<Element, ()> {
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
        let out = PeripheralBuilder::default()
            .name(name.to_owned())
            .base_address(p.address)
            .registers(Some(registers))
            .build()
            .unwrap();

        svd_peripherals.push(out.encode().unwrap());
    }
    println!("Len {}", svd_peripherals.len());

    let mut children = vec![];
    children.push(Element {
        text: Some("Espressif".to_owned()),
        ..Element::new("name")
    });
    children.push(Element {
        children: svd_peripherals,
        ..Element::new("peripherals")
    });
    let out = Element {
        children: children,
        ..Element::new("device")
    };
    Ok(out)
}
