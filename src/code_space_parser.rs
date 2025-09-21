use std::{collections::HashMap, fs::File, io::BufReader, path::PathBuf};
use quick_xml::Reader;
use quick_xml::events::Event;

pub fn parse_code_space(path: PathBuf) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let mut reader = Reader::from_reader(BufReader::new(file));
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut map = HashMap::new();

    let mut in_name = false;
    let mut in_desc = false;
    let mut current_name = String::new();
    let mut current_desc = String::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => {
                match e.local_name().as_ref() {
                    b"name" => in_name = true,
                    b"description" => in_desc = true,
                    _ => {}
                }
            }
            Event::End(e) => {
                match e.local_name().as_ref() {
                    b"name" => in_name = false,
                    b"description" => in_desc = false,
                    b"Definition" => {
                        if !current_name.is_empty() && !current_desc.is_empty() {
                            map.insert(current_name.clone(), current_desc.clone());
                        }
                        current_name.clear();
                        current_desc.clear();
                    }
                    _ => {}
                }
            }
            Event::Text(e) => {
                let txt = e.decode()?.to_string();
                if in_name {
                    current_name = txt;
                } else if in_desc {
                    current_desc = txt;
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(map)
}
