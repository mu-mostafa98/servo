Minimal SVG Tests for Servo Issues

Three test files to reproduce SVG issues:

1. css_inheritance.html - CSS Inheritance
   - Parent div has style="fill: green"
   - SVG text should inherit and be green
   - Current: Text is black (no inheritance)

2. web_fonts.html - Web Fonts
   - SVG has <style> with @import for Roboto font
   - Text should use Roboto web font
   - Current: Uses system fallback font

3. crisp_transforms.html - Crisp Transforms
   - SVG circle scaled 4x with CSS transform
   - Circle should remain crisp when zoomed
   - Current: Becomes blurry (rasterized then scaled)

4. all_tests.html - All three tests in one file
5. simple_all.html - Simplified version of all tests

How to run manually:
  ./mach run svg_tests/all_tests.html
  ./mach run svg_tests/css_inheritance.html
  ./mach run svg_tests/web_fonts.html
  ./mach run svg_tests/crisp_transforms.html
  ./mach run svg_tests/simple_all.html

What to check:
1. Is SVG text green? (should be, likely isn't)
2. Does SVG text use Roboto font? (should, likely doesn't)
3. Is scaled circle crisp when zoomed? (should be, likely blurry)

Root cause: SVG serialized to data URL → rasterized as image → loses CSS inheritance, web fonts, vector scaling.