//! The About window from the app menu and the tray: the icon on a color field drawn from its own
//! colors, a glow behind it that breathes slowly, and the name, the build, and three buttons. It
//! is the OnePlusUI `OnePlusAboutColorField` every samhq app shares, drawn as bitmaps because
//! GPUI has no blur and no radial gradient.

use super::{About, Target, app_icon, panel};
use crate::theme::{self, Appearance, BODY, SECONDARY, StyledType, about as metrics, space};
use crate::ui::{self, Kind};
use crate::{hook, main_window, motion};
use gpui_kit::{
    Animation, AnimationExt as _, AnyWindowHandle, App, AppContext as _, Bounds, Context, Entity,
    FontWeight, Global, IntoElement, ParentElement, Pixels, Render, RenderImage, Styled,
    TitlebarOptions, Window, WindowBounds, WindowOptions, bounce, div, ease_in_out, img, point, px,
    size,
};
use image::{Frame, RgbaImage, imageops};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

/// Where the window was when it closed, so it opens there again.
const POSITION_KEY: &str = "ui.aboutWindowPos";

/// The one About window, while it is open.
struct Open(AnyWindowHandle);

impl Global for Open {}

/// Opens the About window, or brings the open one in front.
pub fn open(about: &Entity<About>, cx: &mut App) {
    hook::record_event("about", "window");
    if let Some(Open(handle)) = cx.try_global::<Open>()
        && cx.windows().contains(handle)
    {
        main_window::bring_forward(*handle, cx);
        return;
    }
    let mac = matches!(hushpen_core::os::Os::current(), hushpen_core::os::Os::Mac);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(placed(about, cx))),
        titlebar: Some(TitlebarOptions {
            title: Some("About Hushpen".into()),
            appears_transparent: mac,
            traffic_light_position: None,
        }),
        is_resizable: false,
        is_minimizable: false,
        app_id: Some(crate::app::APP_ID.into()),
        ..Default::default()
    };
    let about = about.clone();
    let opened = gpui_kit::open_window(options, cx, |window, cx| {
        cx.new(|cx| AboutWindow::new(about, window, cx))
    });
    match opened {
        Ok((handle, _)) => {
            let _ = handle.update(cx, |_, window, _| {
                let native = crate::native::native_handle(window);
                if let Err(error) = hushpen_platform::window::close_button_only(native) {
                    log::warn!("the About window buttons could not be set: {error}");
                }
            });
            cx.set_global(Open(handle));
            main_window::bring_forward(handle, cx);
        }
        Err(error) => log::warn!("the About window could not be opened: {error}"),
    }
}

/// The saved spot when the whole window still fits on a screen, else the middle of the main one.
fn placed(about: &Entity<About>, cx: &App) -> Bounds<Pixels> {
    let window = size(px(metrics::WINDOW_WIDTH), px(metrics::WINDOW_HEIGHT));
    let saved = about
        .read(cx)
        .storage
        .settings
        .get(POSITION_KEY)
        .and_then(|spot| {
            Some(point(
                px(spot["x"].as_f64()? as f32),
                px(spot["y"].as_f64()? as f32),
            ))
        });
    saved
        .map(|origin| Bounds::new(origin, window))
        .filter(|bounds| {
            cx.displays().iter().any(|display| {
                let screen = display.visible_bounds();
                screen.contains(&bounds.origin) && screen.contains(&bounds.bottom_right())
            })
        })
        .unwrap_or_else(|| Bounds::centered(None, window, cx))
}

struct AboutWindow {
    about: Entity<About>,
    art: Art,
}

impl AboutWindow {
    fn new(about: Entity<About>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&about, |_, _, cx| cx.notify()).detach();
        cx.observe_window_activation(window, |_, _, cx| cx.notify())
            .detach();
        theme::observe(window).detach();
        let storage = about.read(cx).storage.clone();
        // shortcut: saved only on close, so a quit with the window open keeps the older spot;
        // save on move if that ever matters.
        window.on_window_should_close(cx, move |window, cx| {
            let origin = window.bounds().origin;
            let spot = json!({"x": f32::from(origin.x), "y": f32::from(origin.y)});
            if let Err(error) = storage.settings.set_internal(POSITION_KEY, spot) {
                log::warn!("the About window position could not be saved: {error}");
            }
            cx.remove_global::<Open>();
            true
        });
        Self {
            about,
            art: Art::draw(crate::assets::APP_ICON_PNG),
        }
    }
}

impl Render for AboutWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let (build, status, can_check, animates) = {
            let me = self.about.read(cx);
            (
                me.build(),
                me.update_status(cx),
                me.can_check_for_updates(cx),
                me.animates(),
            )
        };
        let breathes = animates && motion::animates(cx) && window.is_window_active();
        let field = match palette.appearance {
            Appearance::Dark => self.art.field_dark.clone(),
            Appearance::Light => self.art.field_light.clone(),
        };
        let opens = |target: Target| {
            let about = self.about.clone();
            move |_: &mut Window, cx: &mut App| about.update(cx, |me, cx| me.open(target, cx))
        };
        let check = {
            let about = self.about.clone();
            move |_: &mut Window, cx: &mut App| about.update(cx, |me, cx| me.check_for_updates(cx))
        };
        let buttons = div()
            .flex()
            .gap(px(space::SM))
            .mt(px(metrics::WINDOW_BUTTONS_TOP))
            .child(ui::button(
                palette,
                hook::id("aboutWindow", "checkForUpdates"),
                "Check for Updates",
                Kind::Plain,
                None,
                can_check,
                check,
            ))
            .child(ui::button(
                palette,
                hook::id("aboutWindow", "website"),
                "Website",
                Kind::Plain,
                None,
                true,
                opens(Target::Website),
            ))
            .child(ui::button(
                palette,
                hook::id("aboutWindow", "licenses"),
                "Licenses",
                Kind::Plain,
                None,
                true,
                opens(Target::Licenses),
            ));
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .bg(palette.content)
            .text_color(palette.label)
            .child(header(field, self.art.glow.clone(), breathes))
            .child(
                panel::line(hook::id("aboutWindow", "name"), "Hushpen".to_owned(), BODY)
                    .text_size(px(metrics::WINDOW_TITLE))
                    .font_weight(FontWeight::SEMIBOLD),
            )
            .child(
                panel::line(
                    hook::id("aboutWindow", "tagline"),
                    super::TAGLINE.to_owned(),
                    BODY,
                )
                .mt(px(metrics::TAGLINE_TOP + 1.0))
                .px(px(space::LG))
                .text_color(palette.label_secondary),
            )
            .child(
                div()
                    .flex()
                    .justify_center()
                    .w_full()
                    .px(px(space::LG))
                    .mt(px(metrics::VERSION_TOP + 2.0))
                    .text_token(SECONDARY)
                    .text_color(palette.label_secondary)
                    .child(div().flex_none().child(build.line()))
                    .child(panel::update_status(palette, status)),
            )
            .child(buttons)
            .child(div().flex_1())
            .child(
                panel::line(
                    hook::id("aboutWindow", "copyright"),
                    super::COPYRIGHT.to_owned(),
                    SECONDARY,
                )
                .px(px(space::LG))
                .mb(px(metrics::FOOTER_BOTTOM))
                .text_color(palette.label_secondary),
            )
    }
}

/// The color field with the glow and the icon on its middle.
fn header(field: Arc<RenderImage>, glow: Arc<RenderImage>, breathes: bool) -> impl IntoElement {
    let centered = || {
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
    };
    let (low, high) = metrics::GLOW_OPACITY;
    let rest = low / high;
    let glow = img(glow).size(px(metrics::GLOW)).flex_none();
    let glow = if breathes {
        let breath = Animation::new(Duration::from_secs_f32(2.0 * metrics::GLOW_BREATH))
            .repeat()
            .with_easing(bounce(ease_in_out))
            .with_max_fps(30.0);
        glow.with_animation("about.glow", breath, move |glow, delta| {
            glow.opacity(rest + (1.0 - rest) * delta)
        })
        .into_any_element()
    } else {
        glow.opacity(rest).into_any_element()
    };
    div()
        .relative()
        .w_full()
        .h(px(metrics::FIELD_HEIGHT))
        .flex_none()
        .overflow_hidden()
        .child(img(field).absolute().inset_0().size_full())
        .child(centered().child(glow))
        .child(centered().child(app_icon(metrics::WINDOW_ICON, &metrics::WINDOW_ICON_SHADOW)))
}

/// The bitmaps of the window, drawn once from the icon at twice the point size.
struct Art {
    field_light: Arc<RenderImage>,
    field_dark: Arc<RenderImage>,
    glow: Arc<RenderImage>,
}

/// Bitmap pixels per point.
const SCALE: f32 = 2.0;
/// The field is blurred at a quarter of the bitmap size, which a blur this wide cannot show.
const WORK: f32 = 0.5;

/// A straight color, each channel 0 to 1.
type Tint = [f32; 3];

impl Art {
    fn draw(png: &[u8]) -> Self {
        let icon = image::load_from_memory(png)
            .map(|icon| icon.to_rgba8())
            .unwrap_or_else(|_| RgbaImage::new(1, 1));
        let tints = tints(&icon);
        let blurred = blurred_icon(&icon);
        Self {
            field_light: render(field(&blurred, tints, metrics::FIELD_ICON_OPACITY_LIGHT)),
            field_dark: render(field(&blurred, tints, metrics::FIELD_ICON_OPACITY_DARK)),
            glow: render(glow(tints[0])),
        }
    }
}

/// The two colors of the icon the field and the glow take: the most vivid opaque pixel of an
/// 8 x 8 copy, and the one a fifth of the way down that order (OnePlusUI `tints`).
fn tints(icon: &RgbaImage) -> [Tint; 2] {
    let small = imageops::resize(icon, 8, 8, imageops::FilterType::Triangle);
    let mut colors: Vec<(f32, Tint)> = small
        .pixels()
        .filter(|pixel| f32::from(pixel[3]) / 255.0 > 0.6)
        .map(|pixel| {
            let rgb = [0, 1, 2].map(|channel| f32::from(pixel[channel]) / 255.0);
            let brightness = rgb.iter().copied().fold(0.0, f32::max);
            let low = rgb.iter().copied().fold(1.0, f32::min);
            let saturation = if brightness > 0.0 {
                (brightness - low) / brightness
            } else {
                0.0
            };
            (saturation * brightness, rgb)
        })
        .collect();
    colors.sort_by(|a, b| b.0.total_cmp(&a.0));
    match colors.as_slice() {
        [] => [[0.5; 3]; 2],
        all => [all[0].1, all[all.len() / 5].1],
    }
}

/// The icon at the field size, blurred, premultiplied, centered on a canvas the size of the
/// field, in work pixels.
fn blurred_icon(icon: &RgbaImage) -> RgbaImage {
    let work = |points: f32| (points * WORK).round() as u32;
    let side = work(metrics::FIELD_BLUR_ICON);
    let margin = work(metrics::FIELD_BLUR * 3.0);
    let mut canvas = RgbaImage::new(side + 2 * margin, side + 2 * margin);
    let mut scaled = imageops::resize(icon, side, side, imageops::FilterType::Triangle);
    for pixel in scaled.pixels_mut() {
        let alpha = f32::from(pixel[3]) / 255.0;
        for channel in 0..3 {
            pixel[channel] = (f32::from(pixel[channel]) * alpha).round() as u8;
        }
    }
    imageops::overlay(&mut canvas, &scaled, i64::from(margin), i64::from(margin));
    let blurred = imageops::fast_blur(&canvas, metrics::FIELD_BLUR * WORK);
    let (width, height) = (work(metrics::WINDOW_WIDTH), work(metrics::FIELD_HEIGHT));
    let center = blurred.width() / 2;
    imageops::crop_imm(
        &blurred,
        center - width / 2,
        center - height / 2,
        width,
        height,
    )
    .to_image()
}

/// The field at bitmap size: the blurred icon at `opacity`, the two tints over it, faded out in
/// the lower half. Premultiplied until [`render`].
fn field(blurred: &RgbaImage, tints: [Tint; 2], opacity: f32) -> RgbaImage {
    let (width, height) = (
        (metrics::WINDOW_WIDTH * SCALE) as u32,
        (metrics::FIELD_HEIGHT * SCALE) as u32,
    );
    let base = imageops::resize(blurred, width, height, imageops::FilterType::Triangle);
    RgbaImage::from_fn(width, height, |x, y| {
        let at = base.get_pixel(x, y);
        let mut color = [0, 1, 2, 3].map(|channel| f32::from(at[channel]) / 255.0 * opacity);
        let (px_x, px_y) = (x as f32 / SCALE, y as f32 / SCALE);
        for (tint, (strength, (unit_x, unit_y), radius)) in tints.iter().zip(metrics::FIELD_TINTS) {
            let distance = (px_x - unit_x * metrics::WINDOW_WIDTH)
                .hypot(px_y - unit_y * metrics::FIELD_HEIGHT);
            let alpha = strength * (1.0 - distance / radius).max(0.0);
            for channel in 0..3 {
                color[channel] = tint[channel] * alpha + color[channel] * (1.0 - alpha);
            }
            color[3] = alpha + color[3] * (1.0 - alpha);
        }
        let down = (y as f32 + 0.5) / height as f32;
        let mask = if down <= 0.5 { 1.0 } else { 2.0 * (1.0 - down) };
        image::Rgba(color.map(|value| (value * mask * 255.0).round() as u8))
    })
}

/// The glow at bitmap size: the first tint at the top opacity in the middle, clear at the edge.
/// Premultiplied until [`render`].
fn glow(tint: Tint) -> RgbaImage {
    let side = (metrics::GLOW * SCALE) as u32;
    let radius = side as f32 / 2.0;
    RgbaImage::from_fn(side, side, |x, y| {
        let distance = (x as f32 + 0.5 - radius).hypot(y as f32 + 0.5 - radius);
        let alpha = metrics::GLOW_OPACITY.1 * (1.0 - distance / radius).max(0.0);
        let [r, g, b] = tint.map(|channel| (channel * alpha * 255.0).round() as u8);
        image::Rgba([r, g, b, (alpha * 255.0).round() as u8])
    })
}

/// A premultiplied RGBA bitmap as GPUI wants it: straight alpha, BGRA.
fn render(mut bitmap: RgbaImage) -> Arc<RenderImage> {
    for pixel in bitmap.pixels_mut() {
        let alpha = f32::from(pixel[3]) / 255.0;
        let straight = |value: u8| {
            if alpha > 0.0 {
                (f32::from(value) / alpha).min(255.0).round() as u8
            } else {
                0
            }
        };
        *pixel = image::Rgba([
            straight(pixel[2]),
            straight(pixel[1]),
            straight(pixel[0]),
            pixel[3],
        ]);
    }
    Arc::new(RenderImage::new([Frame::new(bitmap)]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_art_takes_its_colors_from_the_icon_and_fades_out_at_the_bottom() {
        let icon = image::load_from_memory(crate::assets::APP_ICON_PNG)
            .unwrap()
            .to_rgba8();
        let tints = tints(&icon);
        assert_ne!(tints[0], [0.5; 3], "the icon has opaque pixels");
        let field = field(
            &blurred_icon(&icon),
            tints,
            metrics::FIELD_ICON_OPACITY_DARK,
        );
        assert_eq!(field.dimensions(), (760, 460));
        assert!(field.get_pixel(380, 20)[3] > 0, "the top shows color");
        assert_eq!(field.get_pixel(380, 459)[3], 0, "the bottom is clear");
        let glow = glow(tints[0]);
        assert_eq!(glow.get_pixel(0, 0)[3], 0);
        assert!(glow.get_pixel(200, 200)[3] > 130);
    }
}
