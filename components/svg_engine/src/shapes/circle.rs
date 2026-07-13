#[derive(Debug, Clone, Copy)]
pub struct Circle {
    pub cx: f32,
    pub cy: f32,
    pub r: f32,
}

impl crate::shapes::BuildFromElement for Circle {
    fn from_attrs(font_size: f32, attrs: &impl crate::shapes::AttrAccessor) -> Option<Self> {
        use crate::shapes::attr_parsers::parse_length;
        let r = parse_length("r", &|a| attrs.get_attr(a), font_size).ok()?;
        Some(Circle {
            cx: parse_length("cx", &|a| attrs.get_attr(a), font_size).unwrap_or(0.0),
            cy: parse_length("cy", &|a| attrs.get_attr(a), font_size).unwrap_or(0.0),
            r,
        })
    }
}
