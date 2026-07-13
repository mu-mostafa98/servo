use svgtypes::Color as SvgColor;

pub fn parse_css_color(val: &str) -> Option<SvgColor> {
    let val = val.trim();
    if val.eq_ignore_ascii_case("none") || val.eq_ignore_ascii_case("transparent") {
        return None;
    }
    val.parse().ok()
}
