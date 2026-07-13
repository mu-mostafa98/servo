use kurbo::BezPath;

#[derive(Debug, Clone)]
pub struct Path {
    pub path: BezPath,
}

impl crate::shapes::BuildFromElement for Path {
    fn from_attrs(_font_size: f32, attrs: &impl crate::shapes::AttrAccessor) -> Option<Self> {
        let value = attrs.get_attr("d")?;
        let path = BezPath::from_svg(&value).ok()?;
        Some(Path { path })
    }
}
