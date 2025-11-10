use clap::Subcommand;

#[derive(Subcommand)]
pub enum Command {
    Toggle,
    Undo,
    Clear,
    ClearAndDeactivate,
    StrokeWidth { width: f32 },
    StrokeColor { color: csscolorparser::Color },
    Exit,
}

impl Command {
    pub fn serialize(&self) -> Vec<u8> {
        match self {
            Command::Toggle => b"toggle".to_vec(),
            Command::Undo => b"undo".to_vec(),
            Command::Clear => b"clear".to_vec(),
            Command::ClearAndDeactivate => b"clear_and_deactivate".to_vec(),
            Command::StrokeWidth { width } => {
                let s = format!("stroke_width {}", width);
                s.as_bytes().to_vec()
            }
            Command::StrokeColor { color } => {
                let s = format!("stroke_color {}", color.to_css_hex());
                s.as_bytes().to_vec()
            }
            Command::Exit => b"exit".to_vec(),
        }
    }

    pub fn deserialize(s: &[u8]) -> Result<Self, &'static str> {
        let mut split = s.split(|&c| c == b' ');

        match split.next() {
            Some(b"toggle") => Ok(Self::Toggle),
            Some(b"undo") => Ok(Self::Undo),
            Some(b"clear") => Ok(Self::Clear),
            Some(b"clear_and_deactivate") => Ok(Self::ClearAndDeactivate),
            Some(b"stroke_width") => {
                match split
                    .next()
                    .and_then(|width_text| String::from_utf8(width_text.to_vec()).ok())
                    .and_then(|width_text| width_text.parse::<f32>().ok())
                {
                    Some(width) => Ok(Self::StrokeWidth { width: width }),
                    None => Err("received stroke width message but couldn't parse a width"),
                }
            }
            Some(b"stroke_color") => {
                match split
                    .next()
                    .and_then(|color_text| String::from_utf8(color_text.to_vec()).ok())
                    .and_then(|color_text| csscolorparser::parse(&color_text).ok())
                {
                    Some(color) => Ok(Self::StrokeColor { color: color }),
                    None => Err("received stroke color message but couldn't parse a color"),
                }
            }
            Some(b"exit") => Ok(Self::Exit),
            Some(_message) => Err("unknown message"),
            None => Err("received empty message"),
        }
    }
}
