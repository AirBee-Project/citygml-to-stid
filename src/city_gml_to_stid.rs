use crate::code_space_parser::parse_code_space;
use kasane_logic::function::triangle::triangle;
use kasane_logic::id::{SpaceTimeId, coordinates::Point};
use regex::bytes::Regex;
use std::collections::{HashMap, HashSet};

use quick_xml::{events::Event, reader::Reader};
use serde_json::{Value, json};
use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct BuildingInfo {
    pub building_id: String,
    pub stid_set: HashSet<SpaceTimeId>,
    pub attribute_info_map: HashMap<String, String>,
}

/// 最初の bldg:Building の情報を抽出
pub fn first_building_info() -> Result<Option<BuildingInfo>, Box<dyn Error>> {
    let base_dir = Path::new("CityData")
        .join("10201_maebashi-shi_city_2023_citygml_2_op")
        .join("udx")
        .join("bldg");

    let file_path: PathBuf = fs::read_dir(&base_dir)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "gml") {
                Some(path)
            } else {
                None
            }
        })
        .next()
        .ok_or_else(|| format!("No .gml files found in {:?}", base_dir))?;

    let file = File::open(&file_path)?;
    let mut reader = Reader::from_reader(BufReader::new(file));
    reader.config_mut().trim_text(true);

    let mut buf = Vec::<u8>::new();
    let mut in_building = false;
    let mut in_uro = false;
    let mut current_code_space_path: Option<PathBuf> = None;

    let mut building_count = 0;
    let mut buildinginfo = BuildingInfo {
        building_id: String::new(),
        stid_set: HashSet::new(),
        attribute_info_map: HashMap::new(),
    };

    let re_uro = Regex::new(r"^uro:.*$").unwrap();
    let mut current_tag: Option<Vec<u8>> = None;

    loop {
        let ev = reader.read_event_into(&mut buf)?;
        match ev {
            Event::Start(e) => {
                let tag_name: Vec<u8> = e.name().as_ref().to_vec();
                current_tag = Some(tag_name.clone());
                let attrs: Vec<_> = e.attributes().filter_map(|a| a.ok()).collect();

                // bldg:Building 開始
                if tag_name.as_slice() == b"bldg:Building" && !in_building {
                    in_building = true;
                    for a in &attrs {
                        if a.key.as_ref() == b"gml:id" {
                            buildinginfo.building_id = a.unescape_value()?.to_string();
                        }
                    }
                }

                // uro:BuildingDetailAttribute 開始
                if re_uro.is_match(&tag_name) {
                    in_uro = true;
                    current_code_space_path = attrs.iter().find_map(|a| {
                        if a.key.as_ref() == b"codeSpace" {
                            Some(
                                file_path
                                    .parent()
                                    .unwrap_or_else(|| Path::new("."))
                                    .join(a.unescape_value().ok()?.as_ref())
                                    .canonicalize()
                                    .ok()?,
                            )
                        } else {
                            None
                        }
                    });
                }

                // gml:posList, measuredHeight, yearOfConstruction は Text で処理
            }
            Event::Text(t) => {
                if in_building {
                    let text_val = t.decode()?.into_owned();

                    if in_uro {
                        // uro:BuildingDetailAttribute 内のテキスト
                        if let Some(abs_path) = &current_code_space_path {
                            let code_map = parse_code_space(abs_path.clone())?;
                            let name = code_map.get(&text_val).unwrap_or(&text_val);
                            if let Some(tag_bytes) = &current_tag {
                                if let Ok(tag_str) = std::str::from_utf8(tag_bytes) {
                                    buildinginfo
                                        .attribute_info_map
                                        .insert(tag_str.to_string(), name.clone());
                                }
                            }
                        }
                    } else if let Some(tag_name) = &current_tag {
                        if tag_name.as_slice() == b"gml:posList" {
                            // posList のテキストだけ処理
                            let points = parse_points(&text_val)?;
                            buildinginfo
                                .stid_set
                                .extend(citygml_polygon_to_ids(18, &points));
                        }
                    }
                }
            }
            Event::End(e) => {
                let name = e.name();
                let tag_name = name.as_ref();

                if let Some(tag_name) = &current_tag {
                    if tag_name.as_slice() == e.name().as_ref() {
                        current_tag = None;
                    }
                }

                if in_uro && re_uro.is_match(tag_name) {
                    in_uro = false;
                    current_code_space_path = None;
                }

                if in_building && tag_name == b"bldg:Building" {
                    save_building_info_json(building_count, &buildinginfo)?;
                    building_count += 1;
                    in_building = false;
                    in_uro = false;
                    break; // 最初の Building だけ処理
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(Some(buildinginfo))
}

fn citygml_polygon_to_ids(z: u8, vertices: &[Point]) -> HashSet<SpaceTimeId> {
    let mut all_ids = HashSet::new();
    if vertices.len() < 3 {
        return all_ids;
    }
    let a = vertices[0];
    for i in 1..vertices.len() - 1 {
        let b = vertices[i];
        let c = vertices[i + 1];
        all_ids.extend(triangle(z, a, b, c));
    }
    all_ids
}

pub fn parse_points(input: &str) -> Result<Vec<Point>, Box<dyn std::error::Error>> {
    let nums: Vec<f64> = input
        .split_whitespace()
        .map(str::parse::<f64>)
        .collect::<Result<_, _>>()?;
    if nums.len() % 3 != 0 {
        return Err(format!("入力数が3の倍数ではありません: {}", nums.len()).into());
    }
    Ok(nums
        .chunks(3)
        .map(|c| Point {
            latitude: c[0],
            longitude: c[1],
            altitude: c[2],
        })
        .collect())
}

fn save_building_info_json(count: i32, building_info: &BuildingInfo) -> Result<(), Box<dyn Error>> {
    let path = "building_info.json";
    let mut existing: Value = if let Ok(mut f) = File::open(path) {
        let mut buf = String::new();
        f.read_to_string(&mut buf)?;
        if buf.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(&buf)?
        }
    } else {
        json!({})
    };

    existing[&count.to_string()] = json!({
        "id": building_info.building_id,
        "stid_set": building_info.stid_set.iter().map(|stid| stid.to_string()).collect::<Vec<String>>(),
        "attributes": building_info.attribute_info_map
    });

    let mut f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;
    f.write_all(existing.to_string().as_bytes())?;
    Ok(())
}

fn addAttributeInfo() {}
