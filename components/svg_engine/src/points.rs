use kurbo::Point;

/// Parse SVG points attribute (used by polyline and polygon).
/// Format: "x1,y1 x2,y2 ..." or "x1 y1 x2 y2 ..."
pub fn parse_points(value: &str) -> Option<Vec<Point>> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    let mut points = Vec::new();

    // Split by whitespace and comma, but keep comma-separated pairs together
    let tokens: Vec<&str> = value
        .split(|c: char| c.is_ascii_whitespace())
        .filter(|s| !s.is_empty())
        .collect();

    let mut i = 0;
    while i < tokens.len() {
        let token = tokens[i];
        // Try splitting by comma
        if let Some(comma_pos) = token.find(',') {
            let x_str = &token[..comma_pos];
            let y_str = &token[comma_pos + 1..];
            if let (Ok(x), Ok(y)) = (x_str.trim().parse::<f64>(), y_str.trim().parse::<f64>()) {
                points.push(Point::new(x, y));
            }
            i += 1;
        } else if let Ok(x) = token.parse::<f64>() {
            // Bare number — next token should be y
            if i + 1 < tokens.len() {
                if let Ok(y) = tokens[i + 1].parse::<f64>() {
                    points.push(Point::new(x, y));
                    i += 2;
                } else {
                    i += 1;
                }
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    if points.is_empty() { None } else { Some(points) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_space_separated() {
        let pts = parse_points("10,10 20,20 30,30").unwrap();
        assert_eq!(pts.len(), 3);
        assert!((pts[0].x - 10.0).abs() < 0.001);
        assert!((pts[0].y - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_comma_separated() {
        let pts = parse_points("10,10, 20,20, 30,30").unwrap();
        assert_eq!(pts.len(), 3);
        assert!((pts[2].x - 30.0).abs() < 0.001);
    }

    #[test]
    fn test_odd_count() {
        let pts = parse_points("10 20 30");
        assert!(pts.is_none());
    }

    #[test]
    fn test_empty() {
        assert!(parse_points("").is_none());
    }
}
