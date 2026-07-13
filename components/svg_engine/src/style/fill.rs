use svgtypes::Color as SvgColor;

#[derive(Debug, Clone)]
pub struct FillParams {
    pub color: Option<SvgColor>,
    pub opacity: f32,
    pub fill_rule: FillRule,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}
