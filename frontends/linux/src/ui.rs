use gtk4::cairo;
use gtk4::gdk;
use gtk4::pango;
use gtk4::prelude::*;
use gtk4::{
    style_context_add_provider_for_display, Align, Application, ApplicationWindow, Box as GtkBox,
    Button, CssProvider, DrawingArea, GestureClick, Label, Orientation, Overlay, PolicyType,
    ScrolledWindow, STYLE_PROVIDER_PRIORITY_APPLICATION,
};
use std::f64::consts::PI;

const OUTER_MARGIN: i32 = 0;
const VIEW_WIDTH: i32 = 1300;
const VIEW_HEIGHT: i32 = 850;
const FALLBACK_COMMIT: &str = "56d00bf";

pub fn build_ui(app: &Application) {
    load_css();

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Spokenly")
        .default_width(VIEW_WIDTH + OUTER_MARGIN * 2)
        .default_height(VIEW_HEIGHT + OUTER_MARGIN * 2)
        .resizable(false)
        .build();
    window.add_css_class("app-window");

    let root = GtkBox::new(Orientation::Vertical, 0);
    root.add_css_class("window");
    root.set_width_request(VIEW_WIDTH);
    root.set_height_request(VIEW_HEIGHT);

    if OUTER_MARGIN > 0 {
        root.set_margin_top(OUTER_MARGIN);
        root.set_margin_bottom(OUTER_MARGIN);
        root.set_margin_start(OUTER_MARGIN);
        root.set_margin_end(OUTER_MARGIN);
    }

    window.set_child(Some(&root));

    let header = build_window_header(&window);
    root.append(&header);

    let body = GtkBox::new(Orientation::Horizontal, 0);
    body.add_css_class("window-body");

    let sidebar = build_sidebar();
    body.append(&sidebar);

    let main_content = build_dictation_models_page();
    body.append(&main_content);

    root.append(&body);

    window.present();
}

fn load_css() {
    let provider = CssProvider::new();
    provider.load_from_string(APP_CSS);

    if let Some(display) = gdk::Display::default() {
        style_context_add_provider_for_display(
            &display,
            &provider,
            STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn build_window_header(window: &ApplicationWindow) -> GtkBox {
    let header = GtkBox::new(Orientation::Horizontal, 0);
    header.add_css_class("window-header");

    let left = GtkBox::new(Orientation::Horizontal, 0);
    left.add_css_class("window-header-left");
    left.set_width_request(260);
    left.set_hexpand(false);
    left.set_valign(Align::Center);

    let buttons = GtkBox::new(Orientation::Horizontal, 8);
    buttons.add_css_class("window-buttons");
    buttons.set_margin_start(16);

    buttons.append(&make_window_button(window, WindowButton::Close));
    buttons.append(&make_window_button(window, WindowButton::Minimize));
    buttons.append(&make_window_button(window, WindowButton::Maximize));

    left.append(&buttons);

    let right = GtkBox::new(Orientation::Horizontal, 0);
    right.add_css_class("window-header-right");
    right.set_hexpand(true);

    header.append(&left);
    header.append(&right);

    header
}

fn build_sidebar() -> GtkBox {
    let sidebar = GtkBox::new(Orientation::Vertical, 6);
    sidebar.add_css_class("sidebar");
    sidebar.set_width_request(260);
    sidebar.set_vexpand(true);

    for item in NAV_ITEMS {
        sidebar.append(&build_nav_item(item));
    }

    let spacer = GtkBox::new(Orientation::Vertical, 0);
    spacer.set_vexpand(true);
    sidebar.append(&spacer);

    let version_text = option_env!("GIT_COMMIT_HASH").unwrap_or(FALLBACK_COMMIT);
    let version = Label::new(None);
    version.add_css_class("version");
    version.set_margin_top(8);
    version.set_margin_bottom(8);
    version.set_halign(Align::Start);
    version.set_text(&format!("#{version_text}"));
    sidebar.append(&version);

    sidebar
}

fn make_window_button(window: &ApplicationWindow, kind: WindowButton) -> GtkBox {
    let button = GtkBox::new(Orientation::Horizontal, 0);
    button.add_css_class("window-btn");
    button.add_css_class(kind.css_class());
    button.set_size_request(12, 12);
    button.set_halign(Align::Center);
    button.set_valign(Align::Center);

    match kind {
        WindowButton::Close => {
            button.set_cursor_from_name(Some("pointer"));
            let win = window.clone();
            let gesture = GestureClick::new();
            gesture.connect_released(move |_, _, _, _| {
                win.close();
            });
            button.add_controller(gesture);
        }
        WindowButton::Minimize => {
            button.set_cursor_from_name(Some("pointer"));
            let win = window.clone();
            let gesture = GestureClick::new();
            gesture.connect_released(move |_, _, _, _| {
                win.minimize();
            });
            button.add_controller(gesture);
        }
        WindowButton::Maximize => {
            button.add_css_class("disabled");
            button.set_sensitive(false);
        }
    }

    button
}

enum WindowButton {
    Close,
    Minimize,
    Maximize,
}

impl WindowButton {
    fn css_class(&self) -> &'static str {
        match self {
            WindowButton::Close => "close",
            WindowButton::Minimize => "minimize",
            WindowButton::Maximize => "maximize",
        }
    }
}

fn build_nav_item(spec: &NavItemSpec) -> GtkBox {
    let item = GtkBox::new(Orientation::Horizontal, 10);
    item.add_css_class("nav-item");
    if spec.active {
        item.add_css_class("active");
    }
    item.set_margin_bottom(2);
    item.set_cursor_from_name(Some("pointer"));

    let icon = create_icon(spec.icon, 18);
    item.append(&icon);

    let label = Label::new(Some(spec.label));
    label.add_css_class("nav-label");
    label.set_xalign(0.0);
    item.append(&label);

    item
}

fn build_filter_button(spec: &FilterSpec) -> Button {
    let button = Button::new();
    button.add_css_class("filter-button");
    button.add_css_class("flat");
    if spec.active {
        button.add_css_class("active");
    }
    button.set_can_focus(false);
    button.set_focus_on_click(false);
    button.set_has_frame(false);

    let content = GtkBox::new(Orientation::Horizontal, 6);
    content.set_valign(Align::Center);

    let icon = create_icon(spec.icon, 14);
    content.append(&icon);

    let label = Label::new(Some(spec.label));
    label.add_css_class("filter-label");
    label.set_xalign(0.0);
    content.append(&label);

    button.set_child(Some(&content));
    button
}

fn build_dictation_models_page() -> GtkBox {
    let container = GtkBox::new(Orientation::Vertical, 0);
    container.add_css_class("main-content");
    container.set_hexpand(true);
    container.set_vexpand(true);
    container.set_margin_start(48);
    container.set_margin_end(48);
    container.set_margin_top(16);
    container.set_margin_bottom(32);

    let content_header = GtkBox::new(Orientation::Vertical, 0);
    content_header.add_css_class("content-header");

    let header_box = GtkBox::new(Orientation::Vertical, 6);
    header_box.add_css_class("header");

    let title = Label::new(Some("Dictation Models"));
    title.add_css_class("page-title");
    title.set_xalign(0.0);
    header_box.append(&title);

    let subtitle = Label::new(Some("Choose from various dictation models - from cloud-based options to local models that work offline. Select the balance of accuracy, privacy, and speed that works best for your dictation needs."));
    subtitle.add_css_class("page-subtitle");
    subtitle.set_wrap(true);
    subtitle.set_wrap_mode(pango::WrapMode::WordChar);
    subtitle.set_xalign(0.0);
    header_box.append(&subtitle);

    content_header.append(&header_box);

    let filters = GtkBox::new(Orientation::Horizontal, 10);
    filters.add_css_class("filters");
    filters.set_margin_top(24);
    for filter in FILTERS {
        filters.append(&build_filter_button(filter));
    }
    content_header.append(&filters);

    let desc = Label::new(Some("Cloud-based models that require an internet connection. These typically offer higher accuracy but depend on network availability."));
    desc.add_css_class("section-desc");
    desc.set_wrap(true);
    desc.set_wrap_mode(pango::WrapMode::WordChar);
    desc.set_margin_top(16);
    desc.set_xalign(0.0);
    content_header.append(&desc);

    container.append(&content_header);

    let scroller = ScrolledWindow::new();
    scroller.add_css_class("content-body");
    scroller.set_hexpand(true);
    scroller.set_vexpand(true);
    scroller.set_policy(PolicyType::Never, PolicyType::Automatic);
    scroller.set_margin_top(20);

    let models_grid = GtkBox::new(Orientation::Vertical, 14);
    models_grid.add_css_class("models-grid");
    for card in MODEL_CARDS {
        models_grid.append(&build_model_card(card));
    }

    scroller.set_child(Some(&models_grid));
    container.append(&scroller);

    container
}

fn build_model_card(spec: &ModelCardSpec) -> Overlay {
    let overlay = Overlay::new();

    let content = GtkBox::new(Orientation::Horizontal, 18);
    content.add_css_class("model-card");
    if spec.selected {
        content.add_css_class("selected");
    }
    content.set_hexpand(true);

    let icon = build_model_icon(spec.icon_text, spec.icon_variant);
    content.append(&icon);

    let column = GtkBox::new(Orientation::Vertical, 12);
    column.add_css_class("model-content");
    column.set_hexpand(true);

    let header = GtkBox::new(Orientation::Horizontal, 8);
    header.add_css_class("model-header");

    let title = Label::new(Some(spec.title));
    title.add_css_class("model-title");
    title.set_xalign(0.0);
    header.append(&title);

    for badge in spec.badges {
        header.append(&build_badge(*badge));
    }

    column.append(&header);

    let desc_label = match spec.description {
        Description::Plain(text) => {
            let label = Label::new(Some(text));
            label
        }
        Description::Markup(text) => {
            let label = Label::new(Some(text));
            label.set_use_markup(true);
            label
        }
    };
    desc_label.add_css_class("model-desc");
    desc_label.set_wrap(true);
    desc_label.set_wrap_mode(pango::WrapMode::WordChar);
    desc_label.set_xalign(0.0);
    column.append(&desc_label);

    let stats = GtkBox::new(Orientation::Horizontal, 18);
    stats.add_css_class("model-stats");

    if let Some(status_badge) = spec.status_badge {
        stats.append(&build_badge(status_badge));
    }

    for stat in spec.stats {
        stats.append(&build_stat(stat));
    }

    column.append(&stats);

    content.append(&column);

    overlay.set_child(Some(&content));

    if spec.selected {
        let checkmark = Label::new(Some("✓"));
        checkmark.add_css_class("checkmark");
        checkmark.set_halign(Align::End);
        checkmark.set_valign(Align::Start);
        checkmark.set_margin_end(18);
        checkmark.set_margin_top(18);
        overlay.add_overlay(&checkmark);
    }

    overlay
}

fn build_model_icon(text: &str, variant: IconVariant) -> GtkBox {
    let icon = GtkBox::new(Orientation::Vertical, 0);
    icon.add_css_class("model-icon");
    match variant {
        IconVariant::Whisper => icon.add_css_class("model-icon-whisper"),
        IconVariant::Parakeet => icon.add_css_class("model-icon-parakeet"),
    }
    icon.set_size_request(52, 52);
    icon.set_halign(Align::Start);
    icon.set_valign(Align::Center);

    let label = Label::new(Some(text));
    label.add_css_class("model-icon-text");
    label.set_halign(Align::Center);
    label.set_valign(Align::Center);
    icon.append(&label);

    icon
}

fn build_badge(kind: BadgeKind) -> GtkBox {
    let badge = GtkBox::new(Orientation::Horizontal, 6);
    badge.add_css_class("badge");
    badge.set_valign(Align::Center);
    match kind {
        BadgeKind::Fastest => badge.add_css_class("badge-fastest"),
        BadgeKind::Accurate => badge.add_css_class("badge-accurate"),
        BadgeKind::Using => badge.add_css_class("badge-using"),
    }

    if kind == BadgeKind::Using {
        let dot = GtkBox::new(Orientation::Horizontal, 0);
        dot.add_css_class("badge-dot");
        dot.set_valign(Align::Center);
        dot.set_halign(Align::Center);
        badge.append(&dot);
    }

    let label = Label::new(Some(kind.label()));
    label.add_css_class("badge-label");
    label.set_xalign(0.0);
    label.set_valign(Align::Center);
    badge.append(&label);

    badge
}

fn build_stat(spec: &StatSpec) -> GtkBox {
    let stat = GtkBox::new(Orientation::Horizontal, 6);
    stat.add_css_class("stat");
    stat.set_valign(Align::Center);

    let icon = create_icon(spec.icon, 14);
    stat.append(&icon);

    let label = Label::new(Some(spec.label));
    label.add_css_class("stat-label");
    label.set_xalign(0.0);
    label.set_valign(Align::Center);
    stat.append(&label);

    if let Some(dots) = spec.dots {
        let dots_box = GtkBox::new(Orientation::Horizontal, 3);
        dots_box.add_css_class("dots");
        dots_box.set_valign(Align::Center);
        for idx in 0..5 {
            let dot = GtkBox::new(Orientation::Horizontal, 0);
            dot.add_css_class("dot");
            if idx < dots.filled as usize {
                dot.add_css_class("filled");
            }
            dot.set_valign(Align::Center);
            dot.set_halign(Align::Center);
            dot.set_size_request(5, 5);
            dots_box.append(&dot);
        }
        stat.append(&dots_box);
    }

    stat
}

fn create_icon(kind: IconKind, size: i32) -> DrawingArea {
    let area = DrawingArea::new();
    area.add_css_class("icon");
    area.set_content_width(size);
    area.set_content_height(size);
    area.set_halign(Align::Center);
    area.set_valign(Align::Center);

    area.set_draw_func(move |widget, ctx, width, height| {
        #[allow(deprecated)]
        let style = widget.style_context();
        #[allow(deprecated)]
        let color = style.color();
        ctx.set_source_rgba(
            color.red() as f64,
            color.green() as f64,
            color.blue() as f64,
            color.alpha() as f64,
        );
        ctx.set_line_width(2.0);
        ctx.set_line_cap(cairo::LineCap::Round);
        ctx.set_line_join(cairo::LineJoin::Round);

        let width = width as f64;
        let height = height as f64;
        let scale = f64::min(width, height) / 24.0;
        let translate_x = (width - 24.0 * scale) / 2.0;
        let translate_y = (height - 24.0 * scale) / 2.0;

        ctx.translate(translate_x, translate_y);
        ctx.scale(scale, scale);

        draw_icon(ctx, kind);
    });

    area
}

fn draw_icon(ctx: &cairo::Context, kind: IconKind) {
    match kind {
        IconKind::Gear => {
            ctx.arc(12.0, 12.0, 8.5, 0.0, 2.0 * PI);
            let _ = ctx.stroke();

            for i in 0..6 {
                let angle = i as f64 * PI / 3.0 + PI / 6.0;
                let inner = 8.5;
                let outer = 11.0;
                ctx.move_to(12.0 + inner * angle.cos(), 12.0 + inner * angle.sin());
                ctx.line_to(12.0 + outer * angle.cos(), 12.0 + outer * angle.sin());
            }
            let _ = ctx.stroke();

            ctx.arc(12.0, 12.0, 3.0, 0.0, 2.0 * PI);
            let _ = ctx.stroke();
        }
        IconKind::Sliders => {
            ctx.move_to(9.0, 3.0);
            ctx.line_to(9.0, 21.0);
            ctx.move_to(15.0, 3.0);
            ctx.line_to(15.0, 21.0);
            ctx.move_to(5.0, 8.0);
            ctx.line_to(9.0, 8.0);
            ctx.move_to(5.0, 16.0);
            ctx.line_to(9.0, 16.0);
            ctx.move_to(15.0, 12.0);
            ctx.line_to(19.0, 12.0);
            let _ = ctx.stroke();
            ctx.rectangle(11.5, 5.5, 5.0, 5.0);
            ctx.rectangle(7.5, 13.5, 5.0, 5.0);
            let _ = ctx.stroke();
        }
        IconKind::Document => {
            ctx.move_to(6.0, 4.0);
            ctx.line_to(14.0, 4.0);
            ctx.line_to(18.0, 8.0);
            ctx.line_to(18.0, 20.0);
            ctx.line_to(6.0, 20.0);
            ctx.close_path();
            let _ = ctx.stroke();

            ctx.move_to(14.0, 4.0);
            ctx.line_to(14.0, 8.0);
            ctx.line_to(18.0, 8.0);
            let _ = ctx.stroke();
        }
        IconKind::Clock => {
            ctx.arc(12.0, 12.0, 9.5, 0.0, 2.0 * PI);
            let _ = ctx.stroke();

            ctx.move_to(12.0, 7.0);
            ctx.line_to(12.0, 12.0);
            ctx.line_to(16.0, 14.0);
            let _ = ctx.stroke();
        }
        IconKind::Keyboard => {
            ctx.move_to(4.0, 6.0);
            ctx.rectangle(4.0, 6.0, 16.0, 12.0);
            let _ = ctx.stroke();

            for (x, y) in [
                (7.0, 9.0),
                (11.0, 9.0),
                (15.0, 9.0),
                (7.0, 12.0),
                (11.0, 12.0),
                (15.0, 12.0),
            ] {
                ctx.rectangle(x - 0.8, y - 0.8, 1.6, 1.6);
            }
            ctx.rectangle(9.0, 15.0, 6.0, 1.2);
            let _ = ctx.stroke();
        }
        IconKind::Chat => {
            ctx.move_to(20.0, 15.0);
            ctx.curve_to(20.0, 17.209, 18.209, 19.0, 16.0, 19.0);
            ctx.line_to(10.0, 19.0);
            ctx.line_to(8.0, 21.0);
            ctx.line_to(8.0, 19.0);
            ctx.line_to(6.0, 19.0);
            ctx.curve_to(3.791, 19.0, 2.0, 17.209, 2.0, 15.0);
            ctx.line_to(2.0, 5.0);
            ctx.curve_to(2.0, 2.791, 3.791, 1.0, 6.0, 1.0);
            ctx.line_to(18.0, 1.0);
            ctx.curve_to(20.209, 1.0, 22.0, 2.791, 22.0, 5.0);
            ctx.close_path();
            let _ = ctx.stroke();
        }
        IconKind::Info => {
            ctx.arc(12.0, 12.0, 9.0, 0.0, 2.0 * PI);
            let _ = ctx.stroke();

            ctx.move_to(12.0, 10.0);
            ctx.line_to(12.0, 16.0);
            let _ = ctx.stroke();

            ctx.arc(12.0, 7.5, 0.9, 0.0, 2.0 * PI);
            let _ = ctx.fill();
        }
        IconKind::List => {
            for y in [6.0, 12.0, 18.0] {
                ctx.move_to(4.0, y);
                ctx.line_to(20.0, y);
            }
            let _ = ctx.stroke();
        }
        IconKind::Cloud => {
            ctx.move_to(6.0, 18.0);
            ctx.curve_to(3.5, 18.0, 2.0, 15.5, 3.0, 13.0);
            ctx.curve_to(3.8, 11.2, 5.5, 10.2, 7.3, 10.4);
            ctx.curve_to(8.3, 7.0, 11.4, 4.9, 14.8, 5.6);
            ctx.curve_to(17.5, 6.1, 19.6, 8.6, 19.9, 11.3);
            ctx.curve_to(22.2, 11.9, 23.5, 14.4, 22.8, 16.7);
            ctx.curve_to(22.2, 18.8, 20.3, 20.2, 18.1, 20.2);
            ctx.line_to(6.0, 20.2);
            ctx.close_path();
            let _ = ctx.stroke();
        }
        IconKind::Chip => {
            ctx.move_to(6.0, 6.0);
            ctx.line_to(6.0, 4.0);
            ctx.move_to(6.0, 20.0);
            ctx.line_to(6.0, 22.0);
            ctx.move_to(18.0, 6.0);
            ctx.line_to(18.0, 4.0);
            ctx.move_to(18.0, 20.0);
            ctx.line_to(18.0, 22.0);
            ctx.rectangle(4.0, 4.0, 16.0, 16.0);
            let _ = ctx.stroke();
            ctx.arc(12.0, 12.0, 1.5, 0.0, 2.0 * PI);
            let _ = ctx.fill();
        }
        IconKind::Arrows => {
            ctx.move_to(8.0, 6.0);
            ctx.line_to(2.0, 12.0);
            ctx.line_to(8.0, 18.0);
            let _ = ctx.stroke();
            ctx.move_to(16.0, 6.0);
            ctx.line_to(22.0, 12.0);
            ctx.line_to(16.0, 18.0);
            let _ = ctx.stroke();
        }
        IconKind::Bolt => {
            ctx.move_to(13.0, 2.5);
            ctx.line_to(6.5, 13.5);
            ctx.line_to(11.5, 13.5);
            ctx.line_to(11.0, 21.5);
            ctx.line_to(17.5, 10.5);
            ctx.line_to(12.5, 10.5);
            ctx.close_path();
            let _ = ctx.stroke();
        }
        IconKind::Globe => {
            ctx.arc(12.0, 12.0, 9.0, 0.0, 2.0 * PI);
            let _ = ctx.stroke();

            ctx.move_to(3.0, 12.0);
            ctx.line_to(21.0, 12.0);
            let _ = ctx.stroke();

            ctx.move_to(12.0, 3.0);
            ctx.curve_to(15.0, 7.0, 15.0, 17.0, 12.0, 21.0);
            ctx.curve_to(9.0, 17.0, 9.0, 7.0, 12.0, 3.0);
            let _ = ctx.stroke();
        }
    }
}

#[derive(Clone, Copy)]
struct NavItemSpec {
    label: &'static str,
    icon: IconKind,
    active: bool,
}

#[derive(Clone, Copy)]
struct FilterSpec {
    label: &'static str,
    icon: IconKind,
    active: bool,
}

#[derive(Clone, Copy)]
struct ModelCardSpec {
    title: &'static str,
    description: Description,
    icon_text: &'static str,
    icon_variant: IconVariant,
    badges: &'static [BadgeKind],
    status_badge: Option<BadgeKind>,
    stats: &'static [StatSpec],
    selected: bool,
}

#[derive(Clone, Copy)]
struct StatSpec {
    icon: IconKind,
    label: &'static str,
    dots: Option<DotsSpec>,
}

#[derive(Clone, Copy)]
struct DotsSpec {
    filled: u8,
}

#[derive(Clone, Copy)]
enum Description {
    Plain(&'static str),
    Markup(&'static str),
}

#[derive(Clone, Copy)]
enum IconVariant {
    Whisper,
    Parakeet,
}

#[derive(Clone, Copy, PartialEq)]
enum BadgeKind {
    Fastest,
    Accurate,
    Using,
}

impl BadgeKind {
    fn label(&self) -> &'static str {
        match self {
            BadgeKind::Fastest => "Fastest",
            BadgeKind::Accurate => "Most Accurate",
            BadgeKind::Using => "Now Using",
        }
    }
}

#[derive(Clone, Copy)]
enum IconKind {
    Gear,
    Sliders,
    Document,
    Clock,
    Keyboard,
    Chat,
    Info,
    List,
    Cloud,
    Chip,
    Arrows,
    Bolt,
    Globe,
}

const NAV_ITEMS: &[NavItemSpec] = &[
    NavItemSpec {
        label: "General Settings",
        icon: IconKind::Gear,
        active: false,
    },
    NavItemSpec {
        label: "Dictation Models",
        icon: IconKind::Sliders,
        active: true,
    },
    NavItemSpec {
        label: "Transcribe File",
        icon: IconKind::Document,
        active: false,
    },
    NavItemSpec {
        label: "History",
        icon: IconKind::Clock,
        active: false,
    },
    NavItemSpec {
        label: "Keyboard Controls",
        icon: IconKind::Keyboard,
        active: false,
    },
    NavItemSpec {
        label: "AI Prompts",
        icon: IconKind::Chat,
        active: false,
    },
    NavItemSpec {
        label: "About",
        icon: IconKind::Info,
        active: false,
    },
];

const FILTERS: &[FilterSpec] = &[
    FilterSpec {
        label: "All",
        icon: IconKind::List,
        active: false,
    },
    FilterSpec {
        label: "Online",
        icon: IconKind::Cloud,
        active: true,
    },
    FilterSpec {
        label: "Local",
        icon: IconKind::Chip,
        active: false,
    },
    FilterSpec {
        label: "API",
        icon: IconKind::Arrows,
        active: false,
    },
];

const MODEL_CARDS: &[ModelCardSpec] = &[
    ModelCardSpec {
        title: "Whisper Tiny",
        description: Description::Plain(
            "Powered by OpenAI Whisper Tiny - lightweight model optimized for speed. Best for real-time transcription with minimal resource usage.",
        ),
        icon_text: "W",
        icon_variant: IconVariant::Whisper,
        badges: &[BadgeKind::Fastest],
        status_badge: None,
        stats: &[
            StatSpec {
                icon: IconKind::Clock,
                label: "Accuracy",
                dots: Some(DotsSpec { filled: 2 }),
            },
            StatSpec {
                icon: IconKind::Bolt,
                label: "Speed",
                dots: Some(DotsSpec { filled: 5 }),
            },
            StatSpec {
                icon: IconKind::Globe,
                label: "Multilingual",
                dots: None,
            },
        ],
        selected: false,
    },
    ModelCardSpec {
        title: "Whisper Base",
        description: Description::Plain(
            "Powered by OpenAI Whisper Base - balanced model offering good accuracy with reasonable speed. Suitable for most transcription tasks.",
        ),
        icon_text: "W",
        icon_variant: IconVariant::Whisper,
        badges: &[],
        status_badge: None,
        stats: &[
            StatSpec {
                icon: IconKind::Clock,
                label: "Accuracy",
                dots: Some(DotsSpec { filled: 3 }),
            },
            StatSpec {
                icon: IconKind::Bolt,
                label: "Speed",
                dots: Some(DotsSpec { filled: 4 }),
            },
            StatSpec {
                icon: IconKind::Globe,
                label: "Multilingual",
                dots: None,
            },
        ],
        selected: false,
    },
    ModelCardSpec {
        title: "Whisper Small",
        description: Description::Markup(
            "Powered by OpenAI Whisper Small - enhanced accuracy with <b>improved speech recognition</b> for challenging audio. Better than Base for complex transcription.",
        ),
        icon_text: "W",
        icon_variant: IconVariant::Whisper,
        badges: &[],
        status_badge: Some(BadgeKind::Using),
        stats: &[
            StatSpec {
                icon: IconKind::Clock,
                label: "Accuracy",
                dots: Some(DotsSpec { filled: 4 }),
            },
            StatSpec {
                icon: IconKind::Bolt,
                label: "Speed",
                dots: Some(DotsSpec { filled: 3 }),
            },
            StatSpec {
                icon: IconKind::Globe,
                label: "Multilingual",
                dots: None,
            },
        ],
        selected: true,
    },
    ModelCardSpec {
        title: "Parakeet 0.6",
        description: Description::Plain(
            "Powered by NVIDIA Parakeet 0.6B - efficient neural model with strong accuracy for English transcription. Optimized for edge devices.",
        ),
        icon_text: "P",
        icon_variant: IconVariant::Parakeet,
        badges: &[],
        status_badge: None,
        stats: &[
            StatSpec {
                icon: IconKind::Clock,
                label: "Accuracy",
                dots: Some(DotsSpec { filled: 4 }),
            },
            StatSpec {
                icon: IconKind::Bolt,
                label: "Speed",
                dots: Some(DotsSpec { filled: 4 }),
            },
            StatSpec {
                icon: IconKind::Globe,
                label: "English",
                dots: None,
            },
        ],
        selected: false,
    },
    ModelCardSpec {
        title: "Parakeet 1.0",
        description: Description::Plain(
            "Powered by NVIDIA Parakeet 1.1B - advanced neural model with superior accuracy. Larger version provides best-in-class transcription quality for English.",
        ),
        icon_text: "P",
        icon_variant: IconVariant::Parakeet,
        badges: &[BadgeKind::Accurate],
        status_badge: None,
        stats: &[
            StatSpec {
                icon: IconKind::Clock,
                label: "Accuracy",
                dots: Some(DotsSpec { filled: 5 }),
            },
            StatSpec {
                icon: IconKind::Bolt,
                label: "Speed",
                dots: Some(DotsSpec { filled: 3 }),
            },
            StatSpec {
                icon: IconKind::Globe,
                label: "English",
                dots: None,
            },
        ],
        selected: false,
    },
];

const APP_CSS: &str = r#"
window.app-window {
    background: transparent;
}

.window {
    background-color: #1e1e1e;
    border-radius: 12px;
    overflow: hidden;
    color: #e0e0e0;
    font-family: 'Segoe UI', Roboto, 'Noto Sans', sans-serif;
}

.window, .window * {
    -gtk-hint-font-metrics: 0;
    text-rendering: optimizeLegibility;
}

.window-header {
    min-height: 44px;
    background-image: linear-gradient(
        90deg,
        #2a2a2a 0px,
        #2a2a2a 260px,
        #1e1e1e 260px,
        #1e1e1e 100%
    );
    border-top-left-radius: 12px;
    border-top-right-radius: 12px;
}

.window-header-left {
    background-color: transparent;
    display: inline-flex;
    align-items: center;
    border-top-left-radius: 12px;
    min-width: 260px;
    padding: 0 16px;
}

.window-header-right {
    background-color: transparent;
    border-top-right-radius: 12px;
}

.window-buttons {
    gap: 8px;
}

.window-btn {
    border-radius: 999px;
    background-color: #606060;
}

.window-btn.close {
    background-color: #ff5f57;
}

.window-btn.minimize {
    background-color: #ffbd2e;
}

.window-btn.maximize,
.window-btn.maximize.disabled {
    background-color: #3c3c3c;
}

.window-body {
    background-color: transparent;
    flex: 1;
}

.sidebar {
    background-color: #2a2a2a;
    padding: 20px;
    padding-top: 18px;
    border-bottom-left-radius: 12px;
}

.nav-item {
    padding: 10px 12px;
    border-radius: 6px;
    color: #999999;
    font-size: 13px;
    transition: background-color 120ms ease;
    align-items: center;
}

.nav-item:hover {
    background-color: #333333;
}

.nav-item.active {
    background-color: #3b5bdb;
    color: white;
}

.nav-item.active .nav-label {
    color: white;
}

.nav-label {
    color: inherit;
}

.version {
    font-size: 11px;
    color: #6a6a6a;
    letter-spacing: 0.35px;
}

.main-content {
    background-color: #1e1e1e;
    border-bottom-right-radius: 12px;
    display: flex;
    flex-direction: column;
    padding-top: 0;
    padding-bottom: 16px;
}

.page-title {
    font-size: 24px;
    font-weight: 600;
    margin-bottom: 6px;
}

.page-subtitle {
    font-size: 13px;
    color: #888888;
    line-height: 1.5;
}

.filters {
    margin-top: 20px;
    gap: 10px;
    padding-bottom: 4px;
}

.filter-button {
    border-radius: 6px;
    border: 1px solid #3a3a3a;
    background: transparent;
    background-image: none;
    box-shadow: none;
    color: #888888;
    padding: 6px 14px;
    font-size: 12px;
    transition: all 120ms ease;
    min-height: 32px;
}

.filter-button:hover {
    border-color: #555555;
}

.filter-button.active {
    background-color: #3b5bdb;
    border-color: #3b5bdb;
    color: white;
}

.filter-button .icon {
    color: inherit;
}

.section-desc {
    font-size: 12px;
    color: #888888;
    margin-top: 14px;
    line-height: 1.5;
}

.content-body {
    background: transparent;
}

.content-body scrollbar {
    background: transparent;
}

.content-body scrollbar slider {
    background-color: #3a3a3a;
    border-radius: 4px;
}

.models-grid {
    padding-bottom: 12px;
}

.model-card {
    background-color: #252525;
    border: 2px solid #2f2f2f;
    border-radius: 10px;
    padding: 20px;
    transition: border-color 120ms ease, background-color 120ms ease;
}

.model-card:hover {
    border-color: #3a3a3a;
}

.model-card.selected {
    border-color: #3b5bdb;
    background-color: #282d3f;
}

.model-icon {
    border-radius: 10px;
    min-width: 52px;
    min-height: 52px;
    display: flex;
    align-items: center;
    justify-content: center;
}

.model-icon-whisper {
    background-color: #000000;
    color: #ffffff;
}

.model-icon-parakeet {
    background-color: #6366f1;
    color: #ffffff;
}

.model-icon-text {
    font-size: 26px;
    font-weight: 700;
    line-height: 1;
}

.model-title {
    font-size: 15px;
    font-weight: 600;
    color: #e8e8e8;
}

.model-desc {
    font-size: 12px;
    color: #999999;
    line-height: 1.5;
}

.model-stats {
    align-items: center;
    gap: 18px;
}

.stat {
    font-size: 11px;
    color: #aaaaaa;
    align-items: center;
    gap: 6px;
}

.stat-label {
    color: inherit;
}

.icon {
    color: #888888;
}

.model-card.selected .icon {
    color: #cecece;
}

.badge {
    border-radius: 4px;
    font-size: 10px;
    font-weight: 600;
    padding: 3px 9px;
    display: inline-flex;
    align-items: center;
}

.badge-fastest,
.badge-accurate {
    background-color: #3b5bdb;
    color: white;
}

.badge-using {
    background-color: #2f2f2f;
    color: #999999;
    gap: 6px;
}

.badge-dot {
    background-color: #666666;
    border-radius: 999px;
    width: 8px;
    height: 8px;
}

.checkmark {
    background-color: #3b5bdb;
    border-radius: 50%;
    min-width: 22px;
    min-height: 22px;
    color: white;
    font-size: 12px;
    text-align: center;
    padding-top: 2px;
}

.dots {
    gap: 3px;
}

.dot {
    background-color: #3a3a3a;
    border-radius: 999px;
    width: 5px;
    height: 5px;
}

.dot.filled {
    background-color: #dddddd;
}
"#;
