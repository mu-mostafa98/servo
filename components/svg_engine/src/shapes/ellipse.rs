#[derive(Debug, Clone, Copy)]
pub struct Ellipse {
    pub cx: f32,
    pub cy: f32,
    pub rx: f32,
    pub ry: f32,
}

impl crate::shapes::BuildFromElement for Ellipse {
    fn from_attrs(font_size: f32, attrs: &impl crate::shapes::AttrAccessor) -> Option<Self> {
        use crate::shapes::attr_parsers::parse_length;
        let rx = parse_length("rx", &|a| attrs.get_attr(a), font_size).ok()?;
        let ry = parse_length("ry", &|a| attrs.get_attr(a), font_size).ok()?;
        Some(Ellipse {
            cx: parse_length("cx", &|a| attrs.get_attr(a), font_size).unwrap_or(0.0),
            cy: parse_length("cy", &|a| attrs.get_attr(a), font_size).unwrap_or(0.0),
            rx,
            ry,
        })
    }
}
