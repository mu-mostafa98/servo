use kurbo::Point;

#[derive(Debug, Clone)]
pub struct Polygon {
    pub points: Vec<Point>,
}

impl crate::shapes::BuildFromElement for Polygon {
    fn from_attrs(font_size: f32, attrs: &impl crate::shapes::AttrAccessor) -> Option<Self> {
        use crate::shapes::attr_parsers::parse_points;
        parse_points(&|a| attrs.get_attr(a), font_size).ok().map(|pts| Polygon { points: pts })
    }
}
