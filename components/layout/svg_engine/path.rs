use kurbo::{BezPath, PathEl, Point, Vec2};

/// Result of parsing a `d` attribute.
#[derive(Debug, Clone)]
pub struct ParseResult {
    pub path: BezPath,
}

/// Parse an SVG path `d` attribute into a kurbo BezPath.
pub fn parse_path(d: &str) -> Option<BezPath> {
    let d = d.trim();
    if d.is_empty() {
        return None;
    }

    let mut tokens = Tokenizer::new(d);
    let mut path = BezPath::new();
    let mut current_point = Point::new(0.0, 0.0);
    let mut subpath_start = Point::new(0.0, 0.0);
    let mut prev_control = Point::new(0.0, 0.0);
    let mut had_curve = false;

    // We need one command before we can process coordinates
    let mut cmd: Option<char> = None;

    loop {
        // If we don't have a command yet, or if we encounter a new command letter
        if cmd.is_none() {
            match tokens.next_command() {
                Some(c) => cmd = Some(c),
                None => break,
            }
            // Track previous control for smooth curves
            had_curve = false;
        }

        let command = cmd.unwrap();
        let is_relative = command.is_lowercase();
        let base_cmd = command.to_ascii_uppercase();

        match base_cmd {
            'M' => {
                // moveto — 1 or more coordinate pairs
                if let Some((x, y)) = tokens.next_coords(is_relative, current_point) {
                    if is_relative {
                        current_point += Vec2::new(x, y);
                    } else {
                        current_point = Point::new(x, y);
                    }
                    subpath_start = current_point;
                    path.push(PathEl::MoveTo(current_point));
                    // After a moveto, subsequent pairs are lineto
                    cmd = Some(if is_relative { 'l' } else { 'L' });
                } else {
                    cmd = None;
                }
            }
            'Z' => {
                // closepath
                path.push(PathEl::LineTo(subpath_start));
                current_point = subpath_start;
                cmd = None; // need next command
                // Only consume the Z, no arguments
                if !tokens.consume_command() {
                    break;
                }
            }
            'L' => {
                // lineto — 1 or more coordinate pairs
                if let Some((x, y)) = tokens.next_coords(is_relative, current_point) {
                    if is_relative {
                        current_point += Vec2::new(x, y);
                    } else {
                        current_point = Point::new(x, y);
                    }
                    path.push(PathEl::LineTo(current_point));
                } else {
                    cmd = None;
                }
            }
            'H' => {
                // horizontal lineto
                if let Some(v) = tokens.next_number() {
                    let x = if is_relative { current_point.x + v } else { v };
                    current_point = Point::new(x, current_point.y);
                    path.push(PathEl::LineTo(current_point));
                } else {
                    cmd = None;
                }
            }
            'V' => {
                // vertical lineto
                if let Some(v) = tokens.next_number() {
                    let y = if is_relative { current_point.y + v } else { v };
                    current_point = Point::new(current_point.x, y);
                    path.push(PathEl::LineTo(current_point));
                } else {
                    cmd = None;
                }
            }
            'C' => {
                // cubic bezier — 3 coordinate pairs
                if let Some((x1, y1)) = tokens.next_coords(is_relative, current_point) {
                    if let Some((x2, y2)) = tokens.next_coords(is_relative, current_point) {
                        if let Some((x, y)) = tokens.next_coords(is_relative, current_point) {
                            let c1 = if is_relative {
                                current_point + Vec2::new(x1, y1)
                            } else {
                                Point::new(x1, y1)
                            };
                            let c2 = if is_relative {
                                current_point + Vec2::new(x2, y2)
                            } else {
                                Point::new(x2, y2)
                            };
                            let end = if is_relative {
                                current_point + Vec2::new(x, y)
                            } else {
                                Point::new(x, y)
                            };
                            path.push(PathEl::CurveTo(c1, c2, end));
                            prev_control = c2;
                            had_curve = true;
                            current_point = end;
                            continue; // stay in C command for implicit repeats
                        }
                    }
                }
                cmd = None;
            }
            'S' => {
                // smooth cubic bezier — 2 coordinate pairs
                let c1 = if had_curve {
                    current_point + (current_point - prev_control)
                } else {
                    current_point
                };
                if let Some((x2, y2)) = tokens.next_coords(is_relative, current_point) {
                    if let Some((x, y)) = tokens.next_coords(is_relative, current_point) {
                        let c2 = if is_relative {
                            current_point + Vec2::new(x2, y2)
                        } else {
                            Point::new(x2, y2)
                        };
                        let end = if is_relative {
                            current_point + Vec2::new(x, y)
                        } else {
                            Point::new(x, y)
                        };
                        path.push(PathEl::CurveTo(c1, c2, end));
                        prev_control = c2;
                        had_curve = true;
                        current_point = end;
                        continue;
                    }
                }
                cmd = None;
            }
            'Q' => {
                // quadratic bezier — 2 coordinate pairs
                if let Some((x1, y1)) = tokens.next_coords(is_relative, current_point) {
                    if let Some((x, y)) = tokens.next_coords(is_relative, current_point) {
                        let c = if is_relative {
                            current_point + Vec2::new(x1, y1)
                        } else {
                            Point::new(x1, y1)
                        };
                        let end = if is_relative {
                            current_point + Vec2::new(x, y)
                        } else {
                            Point::new(x, y)
                        };
                        prev_control = c;
                        had_curve = true;
                        // Convert quadratic to cubic for kurbo
                        let c1 = current_point + (c - current_point) * (2.0 / 3.0);
                        let c2 = end + (c - end) * (2.0 / 3.0);
                        path.push(PathEl::CurveTo(c1, c2, end));
                        current_point = end;
                        continue;
                    }
                }
                cmd = None;
            }
            'T' => {
                // smooth quadratic bezier — 1 coordinate pair
                let c = if had_curve {
                    current_point + (current_point - prev_control)
                } else {
                    current_point
                };
                if let Some((x, y)) = tokens.next_coords(is_relative, current_point) {
                    let end = if is_relative {
                        current_point + Vec2::new(x, y)
                    } else {
                        Point::new(x, y)
                    };
                    prev_control = c;
                    had_curve = true;
                    let c1 = current_point + (c - current_point) * (2.0 / 3.0);
                    let c2 = end + (c - end) * (2.0 / 3.0);
                    path.push(PathEl::CurveTo(c1, c2, end));
                    current_point = end;
                    continue;
                }
                cmd = None;
            }
            'A' => {
                // elliptical arc — hard: rx ry x-axis-rotation large-arc sweep x y
                if let Some(rx) = tokens.next_number() {
                    if let Some(ry) = tokens.next_number() {
                        if let Some(x_rot) = tokens.next_number() {
                            if let Some(large_arc) = tokens.next_number() {
                                if let Some(sweep) = tokens.next_number() {
                                    if let Some((x, y)) = tokens.next_coords(is_relative, current_point) {
                                        let end = if is_relative {
                                            current_point + Vec2::new(x, y)
                                        } else {
                                            Point::new(x, y)
                                        };
                                        let svg_arc = SvgArc {
                                            from: current_point,
                                            to: end,
                                            radii: Vec2::new(rx.abs(), ry.abs()),
                                            x_rotation: x_rot.to_radians(),
                                            large_arc: large_arc != 0.0,
                                            sweep: sweep != 0.0,
                                        };
                                        if let Some(curves) = svg_arc.to_kurbo() {
                                            for el in curves {
                                                path.push(el);
                                            }
                                        }
                                        current_point = end;
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                }
                cmd = None;
            }
            _ => {
                // Unknown command — skip and move on
                cmd = None;
            }
        }
    }

    if path.is_empty() { None } else { Some(path) }
}

/// Tokenizer for SVG path `d` attribute data.
struct Tokenizer {
    chars: Vec<char>,
    pos: usize,
}

impl Tokenizer {
    fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
        }
    }

    fn skip_whitespace_and_separators(&mut self) {
        while self.pos < self.chars.len() {
            let c = self.chars[self.pos];
            if c.is_ascii_whitespace() || c == ',' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    /// Read the next command letter. Returns None at end of input.
    fn next_command(&mut self) -> Option<char> {
        self.skip_whitespace_and_separators();
        if self.pos >= self.chars.len() {
            return None;
        }
        let c = self.chars[self.pos];
        if c.is_ascii_alphabetic() {
            self.pos += 1;
            Some(c)
        } else {
            // Implicit repeat of previous command — return None
            // so caller uses the stored cmd
            None
        }
    }

    /// Consume the command letter at current position (for Z which has no args).
    /// Returns true if a command was consumed.
    fn consume_command(&mut self) -> bool {
        self.skip_whitespace_and_separators();
        if self.pos < self.chars.len() && self.chars[self.pos].is_ascii_alphabetic() {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// Read the next numeric value.
    fn next_number(&mut self) -> Option<f64> {
        self.skip_whitespace_and_separators();
        if self.pos >= self.chars.len() {
            return None;
        }

        let start = self.pos;

        // Handle leading sign
        if self.pos < self.chars.len() && (self.chars[self.pos] == '-' || self.chars[self.pos] == '+') {
            self.pos += 1;
        }

        // Read integer part
        let mut has_digits = false;
        while self.pos < self.chars.len() && self.chars[self.pos].is_ascii_digit() {
            has_digits = true;
            self.pos += 1;
        }

        // Read fractional part
        if self.pos < self.chars.len() && self.chars[self.pos] == '.' {
            self.pos += 1;
            while self.pos < self.chars.len() && self.chars[self.pos].is_ascii_digit() {
                has_digits = true;
                self.pos += 1;
            }
        }

        // Read exponent
        if self.pos < self.chars.len() && (self.chars[self.pos] == 'e' || self.chars[self.pos] == 'E') {
            self.pos += 1;
            if self.pos < self.chars.len() && (self.chars[self.pos] == '-' || self.chars[self.pos] == '+') {
                self.pos += 1;
            }
            while self.pos < self.chars.len() && self.chars[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
        }

        if !has_digits {
            self.pos = start;
            return None;
        }

        let s: String = self.chars[start..self.pos].iter().collect();
        s.parse::<f64>().ok()
    }

    /// Read the next coordinate pair. For relative commands, x/y are returned
    /// as raw values (caller adds current_point).
    fn next_coords(&mut self, _is_relative: bool, _current: Point) -> Option<(f64, f64)> {
        let x = self.next_number()?;
        let y = self.next_number()?;
        Some((x, y))
    }
}

/// Helper for SVG elliptical arc to cubic bezier conversion.
struct SvgArc {
    from: Point,
    to: Point,
    radii: Vec2,
    x_rotation: f64,
    large_arc: bool,
    sweep: bool,
}

impl SvgArc {
    /// Convert an SVG arc to kurbo PathEl segments using the SVG arc algorithm.
    fn to_kurbo(&self) -> Option<Vec<PathEl>> {
        // Use kurbo's arc-to-bezier conversion via its Arc type
        let center = self.compute_center();
        let (start_angle, _end_angle, angle_delta) = self.compute_angles(&center);
        if angle_delta.abs() < 1e-10 {
            return None;
        }

        let arc = kurbo::Arc {
            center: Point::new(center.x, center.y),
            radii: self.radii,
            start_angle,
            sweep_angle: angle_delta,
            x_rotation: self.x_rotation,
        };

        let mut curves = Vec::new();
        arc.to_cubic_beziers(0.5, |p1, p2, p3| {
            curves.push(PathEl::CurveTo(p1, p2, p3));
        });
        Some(curves)
    }

    /// Compute the center point of the arc using the SVG algorithm.
    fn compute_center(&self) -> Point {
        // Half-distance between start and end points
        let dx = (self.from.x - self.to.x) / 2.0;
        let dy = (self.from.y - self.to.y) / 2.0;
        let cos_r = self.x_rotation.cos();
        let sin_r = self.x_rotation.sin();

        // Step 1: Compute transformed start point (remove rotation)
        let x1 = cos_r * dx + sin_r * dy;
        let y1 = -sin_r * dx + cos_r * dy;

        // Step 2: Ensure radii are large enough
        let mut rx = self.radii.x.abs();
        let mut ry = self.radii.y.abs();
        let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
        if lambda > 1.0 {
            let sqrt_lambda = lambda.sqrt();
            rx *= sqrt_lambda;
            ry *= sqrt_lambda;
        }

        // Step 3: Compute center parameters
        let rx_sq = rx * rx;
        let ry_sq = ry * ry;
        let x1_sq = x1 * x1;
        let y1_sq = y1 * y1;

        let mut sign = 1.0;
        if self.large_arc == self.sweep {
            sign = -1.0;
        }

        let sqrt_val = ((rx_sq * ry_sq) - (rx_sq * y1_sq) - (ry_sq * x1_sq))
            / ((rx_sq * y1_sq) + (ry_sq * x1_sq));
        let sqrt_part = if sqrt_val < 0.0 { 0.0 } else { sqrt_val.sqrt() } * sign;

        let cx1 = sqrt_part * rx * y1 / ry;
        let cy1 = sqrt_part * (-ry) * x1 / rx;

        // Step 4: Transform back
        let cx = cos_r * cx1 - sin_r * cy1 + (self.from.x + self.to.x) / 2.0;
        let cy = sin_r * cx1 + cos_r * cy1 + (self.from.y + self.to.y) / 2.0;

        Point::new(cx, cy)
    }

    fn compute_angles(&self, center: &Point) -> (f64, f64, f64) {
        let cos_r = self.x_rotation.cos();
        let sin_r = self.x_rotation.sin();

        let x1 = (cos_r * (self.from.x - center.x) + sin_r * (self.from.y - center.y)) / self.radii.x;
        let y1 = (-sin_r * (self.from.x - center.x) + cos_r * (self.from.y - center.y)) / self.radii.y;
        let x2 = (cos_r * (self.to.x - center.x) + sin_r * (self.to.y - center.y)) / self.radii.x;
        let y2 = (-sin_r * (self.to.x - center.x) + cos_r * (self.to.y - center.y)) / self.radii.y;

        let start_angle = y1.atan2(x1);
        let end_angle = y2.atan2(x2);

        let mut angle_delta = end_angle - start_angle;

        if self.sweep && angle_delta < 0.0 {
            angle_delta += std::f64::consts::TAU;
        } else if !self.sweep && angle_delta > 0.0 {
            angle_delta -= std::f64::consts::TAU;
        }

        (start_angle, end_angle, angle_delta)
    }
}
