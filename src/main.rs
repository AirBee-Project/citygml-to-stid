pub mod city_gml_to_stid;
pub mod code_space_parser;
use crate::city_gml_to_stid::first_building_info;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let path = "54393087_bldg_6697_op.gml";
    if let Some(info) = first_building_info()? { println!("{:#?}", info) }
    Ok(())
}
