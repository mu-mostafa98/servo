use euclid::Transform2D;

/// Parse an SVG transform attribute string into a 2D transform matrix.
/// Supports: matrix(a,b,c,d,e,f), translate(tx,ty), rotate(a), scale(sx,sy), skewX(a), skewY(a)
pub fn parse_transform(value: &str) -> Option<Transform2D<f32, (), ()>> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    let mut result = Transform2D::identity();

    let mut pos = 0;
    let chars: Vec<char> = value.chars().collect();

    while pos < chars.len() {
        // Skip whitespace/comma
        while pos < chars.len() && (chars[pos].is_ascii_whitespace() || chars[pos] == ',') {
            pos += 1;
        }
        if pos >= chars.len() {
            break;
        }

        // Read function name
        let start = pos;
        while pos < chars.len() && chars[pos].is_ascii_alphabetic() {
            pos += 1;
        }
        let func_name: String = chars[start..pos].iter().collect();

        // Skip whitespace
        while pos < chars.len() && chars[pos].is_ascii_whitespace() {
            pos += 1;
        }

        // Expect '('
        if pos >= chars.len() || chars[pos] != '(' {
            break;
        }
        pos += 1; // skip '('

        // Parse arguments
        let mut args: Vec<f32> = Vec::new();
        loop {
            // Skip whitespace/comma
            while pos < chars.len() && (chars[pos].is_ascii_whitespace() || chars[pos] == ',') {
                pos += 1;
            }
            if pos >= chars.len() || chars[pos] == ')' {
                break;
            }

            // Parse number
            let num_start = pos;
            if pos < chars.len() && (chars[pos] == '-' || chars[pos] == '+') {
                pos += 1;
            }
            let mut has_dot = false;
            while pos < chars.len() && (chars[pos].is_ascii_digit() || chars[pos] == '.') {
                if chars[pos] == '.' {
                    if has_dot {
                        break;
                    }
                    has_dot = true;
                }
                pos += 1;
            }
            if pos == num_start || (pos == num_start + 1 && (chars[num_start] == '-' || chars[num_start] == '+')) {
                break; // no digits
            }
            let num_str: String = chars[num_start..pos].iter().collect();
            if let Ok(n) = num_str.parse::<f32>() {
                args.push(n);
            } else {
                break;
            }
        }

        // Expect ')'
        while pos < chars.len() && (chars[pos].is_ascii_whitespace() || chars[pos] == ',') {
            pos += 1;
        }
        if pos < chars.len() && chars[pos] == ')' {
            pos += 1;
        }

        // Apply the transform
        match func_name.as_str() {
            "matrix" => {
                if args.len() >= 6 {
                    let m = Transform2D::new(
                        args[0], args[1], args[2], args[3], args[4], args[5],
                    );
                    result = result.then(&m);
                }
            }
            "translate" => {
                let tx = args.first().copied().unwrap_or(0.0);
                let ty = args.get(1).copied().unwrap_or(0.0);
                let m = Transform2D::translation(tx, ty);
                result = result.then(&m);
            }
            "rotate" => {
                if let Some(angle) = args.first() {
                    let rad = angle.to_radians();
                    let cos = rad.cos();
                    let sin = rad.sin();
                    let m = Transform2D::new(cos, sin, -sin, cos, 0.0, 0.0);
                    result = result.then(&m);
                }
            }
            "scale" => {
                let sx = args.first().copied().unwrap_or(1.0);
                let sy = args.get(1).copied().unwrap_or(sx);
                let m = Transform2D::scale(sx, sy);
                result = result.then(&m);
            }
            "skewX" => {
                if let Some(angle) = args.first() {
                    let tan = angle.to_radians().tan();
                    let m = Transform2D::new(1.0, 0.0, tan, 1.0, 0.0, 0.0);
                    result = result.then(&m);
                }
            }
            "skewY" => {
                if let Some(angle) = args.first() {
                    let tan = angle.to_radians().tan();
                    let m = Transform2D::new(1.0, tan, 0.0, 1.0, 0.0, 0.0);
                    result = result.then(&m);
                }
            }
            _ => {}
        }
    }

    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translate() {
        let t = parse_transform("translate(10, 20)").unwrap();
        let p = t.transform_point(&euclid::point2(0.0, 0.0));
        assert!((p.x - 10.0).abs() < 0.001);
        assert!((p.y - 20.0).abs() < 0.001);
    }

    #[test]
    fn test_scale() {
        let t = parse_transform("scale(2)").unwrap();
        let p = t.transform_point(&euclid::point2(5.0, 5.0));
        assert!((p.x - 10.0).abs() < 0.001);
        assert!((p.y - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_rotate() {
        let t = parse_transform("rotate(90)").unwrap();
        let p = t.transform_point(&euclid::point2(1.0, 0.0));
        assert!((p.x - 0.0).abs() < 0.001);
        assert!((p.y - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_matrix() {
        let t = parse_transform("matrix(1, 0, 0, 1, 10, 10)").unwrap();
        let p = t.transform_point(&euclid::point2(5.0, 5.0));
        assert!((p.x - 15.0).abs() < 0.001);
        assert!((p.y - 15.0).abs() < 0.001);
    }

    #[test]
    fn test_combined() {
        let t = parse_transform("translate(10, 0) scale(2)").unwrap();
        let p = t.transform_point(&euclid::point2(5.0, 0.0));
        assert!((p.x - 20.0).abs() < 0.001);
    }
}
