use embedded_graphics::{
    mono_font::{ascii::FONT_8X13, MonoTextStyle},
    pixelcolor::Rgb888,
    prelude::*,
    primitives::{PrimitiveStyleBuilder, Rectangle},
    text::{Alignment, Baseline, Text, TextStyleBuilder},
};

// ---------------------------------------------------------------------------
// DrawTarget adapter
// ---------------------------------------------------------------------------

/// Wraps the `pixels` RGBA framebuffer so embedded-graphics can draw into it.
struct PixelBuffer<'a> {
    frame: &'a mut [u8],
    width: u32,
    height: u32,
}

impl<'a> PixelBuffer<'a> {
    fn new(frame: &'a mut [u8], width: u32, height: u32) -> Self {
        Self { frame, width, height }
    }
}

impl DrawTarget for PixelBuffer<'_> {
    type Color = Rgb888;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            let x = point.x;
            let y = point.y;
            if x >= 0 && y >= 0 && (x as u32) < self.width && (y as u32) < self.height {
                let i = ((y as u32 * self.width + x as u32) * 4) as usize;
                self.frame[i]     = color.r();
                self.frame[i + 1] = color.g();
                self.frame[i + 2] = color.b();
                // byte [i+3] is alpha — the ray tracer already wrote 255 there
            }
        }
        Ok(())
    }
}

impl OriginDimensions for PixelBuffer<'_> {
    fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }
}

// ---------------------------------------------------------------------------
// Button
// ---------------------------------------------------------------------------

/// A labelled, clickable button drawn directly into the pixel framebuffer.
///
/// Usage:
/// ```
/// let mut btn = Button::new("ORBIT: ON", 10, 10, 110, 26);
/// // in event handler:
/// if btn.contains(cx, cy) { btn.active = !btn.active; }
/// // in render loop (after scene.render, before pixels.render):
/// btn.draw(frame, WIDTH, HEIGHT);
/// ```
pub struct Button {
    /// Text displayed on the button face.
    pub label: String,
    /// Top-left corner, in screen pixels.
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    /// Visual state: green when true, red when false.
    pub active: bool,
}

impl Button {
    /// Creates a new button.  `active` defaults to `true`.
    pub fn new(label: impl Into<String>, x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            label: label.into(),
            x, y, width, height,
            active: true,
        }
    }

    /// Returns `true` if the screen point `(cx, cy)` falls inside this button.
    pub fn contains(&self, cx: f64, cy: f64) -> bool {
        cx >= self.x as f64
            && cx < (self.x + self.width as i32) as f64
            && cy >= self.y as f64
            && cy < (self.y + self.height as i32) as f64
    }

    /// Draws the button into `frame` (RGBA, row-major, `screen_w × screen_h` pixels).
    pub fn draw(&self, frame: &mut [u8], screen_w: u32, screen_h: u32) {
        let mut display = PixelBuffer::new(frame, screen_w, screen_h);

        // Background — green (active) or red (inactive).
        let bg = if self.active {
            Rgb888::new(30, 160, 30)
        } else {
            Rgb888::new(160, 30, 30)
        };
        Rectangle::new(Point::new(self.x, self.y), Size::new(self.width, self.height))
            .into_styled(PrimitiveStyleBuilder::new().fill_color(bg).build())
            .draw(&mut display)
            .unwrap();

        // White border.
        Rectangle::new(Point::new(self.x, self.y), Size::new(self.width, self.height))
            .into_styled(
                PrimitiveStyleBuilder::new()
                    .stroke_color(Rgb888::WHITE)
                    .stroke_width(1)
                    .build(),
            )
            .draw(&mut display)
            .unwrap();

        // Centred label (horizontally and vertically).
        let char_style = MonoTextStyle::new(&FONT_8X13, Rgb888::WHITE);
        let text_style = TextStyleBuilder::new()
            .alignment(Alignment::Center)
            .baseline(Baseline::Middle)
            .build();
        let cx = self.x + self.width as i32 / 2;
        let cy = self.y + self.height as i32 / 2;
        Text::with_text_style(&self.label, Point::new(cx, cy), char_style, text_style)
            .draw(&mut display)
            .unwrap();
    }
}
