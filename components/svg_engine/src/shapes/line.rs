#[derive(Debug, Clone, Copy)]
pub struct Line {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl crate::shapes::BuildFromElement for Line {
    fn from_attrs(font_size: f32, attrs: &impl crate::shapes::AttrAccessor) -> Option<Self> {
        use crate::shapes::attr_parsers::parse_length;
        Some(Line {
            x1: parse_length("x1", &|a| attrs.get_attr(a), font_size).unwrap_or(0.0),
            y1: parse_length("y1", &|a| attrs.get_attr(a), font_size).unwrap_or(0.0),
            x2: parse_length("x2", &|a| attrs.get_attr(a), font_size).unwrap_or(0.0),
            y2: parse_length("y2", &|a| attrs.get_attr(a), font_size).unwrap_or(0.0),
        })
    }
}
