use app_units::Au;
use malloc_size_of_derive::MallocSizeOf;

/// A parsed SVG length value with its unit type.
#[derive(Debug, Clone, Copy, PartialEq, MallocSizeOf)]
pub enum SvgLength {
    /// A value in CSS/SVG px units (or unitless → interpreted as px).
    Px(f32),
    /// A percentage of the viewport or containing block dimension.
    Percent(f32),
}

impl SvgLength {
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.is_empty() {
            return None;
        }

        // Try percentage
        if let Some(percent_str) = value.strip_suffix('%') {
            let num: f32 = percent_str.trim().parse().ok()?;
            return Some(SvgLength::Percent(num / 100.0));
        }

        // Try unit suffixes
        let unitless = value.trim_end_matches(|c: char| c.is_alphabetic() || c == '%');
        let unit = &value[unitless.len()..];

        let num: f32 = unitless.trim().parse().ok()?;

        match unit {
            "" | "px" => Some(SvgLength::Px(num)),
            "pt" => Some(SvgLength::Px(num * 1.33333)),   // 1pt = 1/72 inch ≈ 1.333px
            "pc" => Some(SvgLength::Px(num * 16.0)),       // 1pc = 12pt = 16px
            "cm" => Some(SvgLength::Px(num * 37.7952756)), // 1cm ≈ 37.8px (96dpi)
            "mm" => Some(SvgLength::Px(num * 3.77952756)), // 1mm ≈ 3.78px
            "in" => Some(SvgLength::Px(num * 96.0)),       // 1in = 96px
            "em" => {
                // Relative to font-size — caller must resolve against parent
                // Return raw value for external resolution
                Some(SvgLength::Px(num * 16.0)) // fallback 16px
            }
            "ex" => Some(SvgLength::Px(num * 8.0)), // ~0.5em fallback
            _ => {
                // Unknown unit — treat as px per SVG spec fallback
                Some(SvgLength::Px(num))
            }
        }
    }

    /// Resolve this length against a reference size (for percentages).
    pub fn resolve(&self, reference_length: f32) -> f32 {
        match self {
            SvgLength::Px(v) => *v,
            SvgLength::Percent(p) => p * reference_length,
        }
    }

    /// Resolve to Au for layout integration.
    pub fn to_au(&self, reference_length: f32) -> Au {
        Au::from_f32_px(self.resolve(reference_length))
    }

    /// Get raw pixel value (only for non-percentage lengths).
    pub fn px(&self) -> Option<f32> {
        match self {
            SvgLength::Px(v) => Some(*v),
            SvgLength::Percent(_) => None,
        }
    }
}

/// Parse a pair of comma/whitespace-separated coordinates.
pub fn parse_coordinate_pair(value: &str) -> Option<(SvgLength, SvgLength)> {
    let parts: Vec<&str> = value
        .split(|c: char| c == ',' || c == ' ')
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() >= 2 {
        let x = SvgLength::parse(parts[0])?;
        let y = SvgLength::parse(parts[1])?;
        Some((x, y))
    } else if parts.len() == 1 {
        let x = SvgLength::parse(parts[0])?;
        Some((x, x))
    } else {
        None
    }
}

/// Parse a whitespace-separated list of numbers.
pub fn parse_number_list(value: &str) -> Vec<f32> {
    value
        .split(|c: char| c == ',' || c == ' ' || c == ';')
        .filter_map(|s| {
            let s = s.trim();
            if s.is_empty() {
                None
            } else {
                s.parse::<f32>().ok()
            }
        })
        .collect()
}
