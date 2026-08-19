use gtk::prelude::*;
use crate::network;

#[derive(Clone, Debug)]
pub struct DscVerificationResult {
    pub dsc_status: String,
    pub c2pa_status: String,
    pub manifest_title: String,
    pub provenance: String,
    pub actions: String,
    pub claim_generator: String,
    pub digital_source_type: String,
}

pub struct PlayerUi {
    pub container: gtk::Box,
    video_box: gtk::Box,
    dsc_icon: gtk::Box,
    c2pa_icon: gtk::Box,
    dsc_status: gtk::Label,
    c2pa_status: gtk::Label,
    source_type: gtk::Label,
    manifest_title: gtk::Label,
    manifest_generator: gtk::Label,
    manifest_provenance: gtk::Label,
    manifest_actions: gtk::Label,
}

impl PlayerUi {
    pub fn new(window_width: i32, window_height: i32, whep_endpoint: &str) -> Self {
        // CSS
        let css = gtk::CssProvider::new();
        css.load_from_data(
            ".status-wait { background: #888888; border-radius: 3px; } \
             .status-ok    { background: #00aa00; border-radius: 3px; } \
             .status-fail  { background: #cc0000; border-radius: 3px; } \
             .icon-frame   { padding: 4px; margin: 4px; } \
             .section-title { font-size: 14px; font-weight: bold; margin-top: 12px; } \
             .data-label   { font-size: 13px; font-family: monospace; margin: 2px 0; } \
             .footer-text  { font-size: 11px; color: #888888; } \
             .header-title { font-size: 18px; font-weight: bold; }",
        );
        gtk::style_context_add_provider_for_display(
            &gdk::Display::default().expect("No display"),
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        // Header
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        header.set_margin_start(16);
        header.set_margin_end(16);
        header.set_margin_top(8);
        header.set_margin_bottom(8);

        let is_dark = std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "color-scheme"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.contains("dark") || s.contains("prefer-dark"))
            .unwrap_or(false);
        let logo_path = if is_dark {
            "assets/logo-fluendo.png"
        } else {
            "assets/logo-fluendo-positive.png"
        };
        if let Ok(texture) = gdk::Texture::from_filename(logo_path) {
            let logo = gtk::Picture::for_paintable(&texture);
            logo.set_can_shrink(true);
            logo.set_size_request(100, 22);
            header.append(&logo);
        }
        let title = gtk::Label::builder()
            .label("C2PA-DSC — WebRTC Broadcasting Showcase")
            .css_classes(["header-title"])
            .build();
        header.append(&title);

        // Footer
        let footer = gtk::Label::builder()
            .label("© 2026 Fraunhofer HHI & Fluendo S.A.  |  Powered by GStreamer")
            .css_classes(["footer-text"])
            .halign(gtk::Align::Center)
            .margin_top(8)
            .margin_bottom(8)
            .build();

        // Video area (left)
        let video_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        video_box.set_size_request(window_width * 45 / 100, window_height - 80);
        video_box.set_hexpand(false);
        video_box.set_vexpand(true);

        let video_placeholder = gtk::Label::builder()
            .label("Video will appear here\n(ximagesink or gtk4paintablesink)")
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .vexpand(true)
            .build();
        video_box.append(&video_placeholder);

        // Right info panel
        let right_panel = gtk::Box::new(gtk::Orientation::Vertical, 8);
        right_panel.set_margin_start(16);
        right_panel.set_margin_end(16);
        right_panel.set_vexpand(true);
        right_panel.set_hexpand(true);

        // Validation Status section
        let sec_status = section_label("Validation Status");

        let dsc_texture = icon_texture("assets/dsc.png");
        let c2pa_texture = icon_texture("assets/c2pa.jpg");

        let dsc_icon = status_icon(&dsc_texture, "status-wait");
        let c2pa_icon = status_icon(&c2pa_texture, "status-wait");

        let icon_row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        icon_row.append(&dsc_icon);
        icon_row.append(&c2pa_icon);

        let dsc_status = gtk::Label::builder().label("").css_classes(["data-label"]).halign(gtk::Align::Start).build();
        let c2pa_status = gtk::Label::builder().label("").css_classes(["data-label"]).halign(gtk::Align::Start).build();

        // C2PA Manifest section
        let sec_manifest = section_label("C2PA Manifest");
        let source_type = gtk::Label::builder().label("—").css_classes(["data-label"]).halign(gtk::Align::Start).build();
        let manifest_title = gtk::Label::builder().label("—").css_classes(["data-label"]).halign(gtk::Align::Start).wrap(true).build();
        let manifest_generator = gtk::Label::builder().label("—").css_classes(["data-label"]).halign(gtk::Align::Start).build();
        let manifest_provenance = gtk::Label::builder().label("—").css_classes(["data-label"]).halign(gtk::Align::Start).build();
        let manifest_actions = gtk::Label::builder().label("—").css_classes(["data-label"]).halign(gtk::Align::Start).build();

        let manifest_grid = gtk::Grid::builder()
            .column_spacing(12)
            .row_spacing(4)
            .margin_start(8)
            .build();
        manifest_grid.attach(&bold_label("Source Type:"), 0, 0, 1, 1);
        manifest_grid.attach(&source_type, 1, 0, 1, 1);
        manifest_grid.attach(&bold_label("Title:"), 0, 1, 1, 1);
        manifest_grid.attach(&manifest_title, 1, 1, 1, 1);
        manifest_grid.attach(&bold_label("Claim Gen:"), 0, 2, 1, 1);
        manifest_grid.attach(&manifest_generator, 1, 2, 1, 1);
        manifest_grid.attach(&bold_label("Provenance:"), 0, 3, 1, 1);
        manifest_grid.attach(&manifest_provenance, 1, 3, 1, 1);
        manifest_grid.attach(&bold_label("Actions:"), 0, 4, 1, 1);
        manifest_grid.attach(&manifest_actions, 1, 4, 1, 1);

        // Connection info
        let local_ip = network::local_ip();
        let server_addr = whep_endpoint
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .trim_end_matches("/whep/endpoint");
        let sec_info = section_label("Connection");
        let info_text = gtk::Label::builder()
            .label(&format!("Player IP: {}\nWHEP Server: {}", local_ip, server_addr))
            .css_classes(["data-label"])
            .halign(gtk::Align::Start)
            .build();

        // Assemble right panel
        right_panel.append(&sec_status);
        right_panel.append(&icon_row);
        right_panel.append(&dsc_status);
        right_panel.append(&c2pa_status);
        right_panel.append(&sec_manifest);
        right_panel.append(&manifest_grid);
        right_panel.append(&sec_info);
        right_panel.append(&info_text);

        // Main layout
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        content.append(&video_box);
        content.append(&right_panel);

        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        container.append(&header);
        container.append(&content);
        container.append(&footer);

        Self {
            container,
            video_box,
            dsc_icon,
            c2pa_icon,
            dsc_status,
            c2pa_status,
            source_type,
            manifest_title,
            manifest_generator,
            manifest_provenance,
            manifest_actions,
        }
    }

    pub fn set_video_widget(&self, widget: &impl gtk::prelude::IsA<gtk::Widget>) {
        while let Some(child) = self.video_box.first_child() {
            self.video_box.remove(&child);
        }
        self.video_box.append(widget);
    }

    pub fn update(&self, result: &DscVerificationResult) {
        let dsc_class = match result.dsc_status.as_str() {
            "valid" => "status-ok",
            "invalid" => "status-fail",
            _ => "status-wait",
        };
        self.dsc_icon.set_css_classes(&[dsc_class, "icon-frame"]);
        self.dsc_status.set_markup(&format!("<span size='small'>dsc-status: <tt>{}</tt></span>", result.dsc_status));

        let c2pa_class = if result.c2pa_status == "valid" {
            "status-ok"
        } else if result.c2pa_status.is_empty() || result.c2pa_status == "unknown" {
            "status-wait"
        } else {
            "status-fail"
        };
        self.c2pa_icon.set_css_classes(&[c2pa_class, "icon-frame"]);
        self.c2pa_status.set_markup(&format!("<span size='small'>c2pa-status: <tt>{}</tt></span>", result.c2pa_status));

        if result.c2pa_status == "valid" {
            let (label, color) = source_type_label(&result.digital_source_type);
            self.source_type.set_markup(&format!("<span foreground='{}'>{}</span>", color, label));
        } else if result.c2pa_status.is_empty() || result.c2pa_status == "unknown" {
            self.source_type.set_markup("<span foreground='#888888'>Waiting…</span>");
        } else {
            self.source_type.set_markup("<span foreground='#888888'>Unverified</span>");
        }

        if result.c2pa_status == "valid" && !result.manifest_title.is_empty() {
            self.manifest_title.set_label(&result.manifest_title);
            self.manifest_generator.set_label(&result.claim_generator);
            self.manifest_provenance.set_label(&format_provenance(&result.provenance));
            self.manifest_actions.set_label(&format_actions(&result.actions));
        } else if result.c2pa_status != "valid" && !result.manifest_title.is_empty() {
            self.manifest_title.set_markup(&format!("<span foreground='#cc0000'>{} (UNVERIFIED)</span>", result.manifest_title));
            self.manifest_generator.set_label("\u{2014}");
            self.manifest_provenance.set_label("\u{2014}");
            self.manifest_actions.set_label("\u{2014}");
        } else {
            self.manifest_title.set_label("\u{2014}");
            self.manifest_generator.set_label("\u{2014}");
            self.manifest_provenance.set_label("\u{2014}");
            self.manifest_actions.set_label("\u{2014}");
        }
    }
}

fn source_type_label(digital_source_types: &str) -> (&'static str, &'static str) {
    if digital_source_types.contains("trainedAlgorithmicMedia") {
        ("AI-modified", "#d97706")
    } else if digital_source_types.contains("digitalCapture") {
        ("Webcam (direct)", "#00aa00")
    } else {
        ("Unknown", "#888888")
    }
}

fn format_provenance(json: &str) -> String {
    if json.is_empty() || json == "[]" {
        return "\u{2014}".to_string();
    }
    if let Ok(vals) = serde_json::from_str::<Vec<serde_json::Value>>(json) {
        let items: Vec<String> = vals
            .iter()
            .filter_map(|v| {
                let title = v.get("title").and_then(|t| t.as_str()).unwrap_or("?");
                let format = v.get("format").and_then(|f| f.as_str()).unwrap_or("");
                if format.is_empty() {
                    Some(title.to_string())
                } else {
                    Some(format!("{} ({})", title, format))
                }
            })
            .collect();
        if items.is_empty() { "\u{2014}".to_string() } else { items.join(", ") }
    } else {
        "\u{2014}".to_string()
    }
}

fn short_source_type(uri: &str) -> &str {
    uri.rsplit('/').next().unwrap_or(uri)
}

fn format_actions(json: &str) -> String {
    if json.is_empty() || json == "[]" {
        return "\u{2014}".to_string();
    }
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(json) {
        if let Some(actions) = val.get("actions").and_then(|a| a.as_array()) {
            let items: Vec<String> = actions
                .iter()
                .filter_map(|a| {
                    let action = a.get("action").and_then(|s| s.as_str())?;
                    let source = a
                        .get("digitalSourceType")
                        .and_then(|s| s.as_str())
                        .map(short_source_type)
                        .unwrap_or("\u{2014}");
                    Some(format!("{action}: {source}"))
                })
                .collect();
            if items.is_empty() { "\u{2014}".to_string() } else { items.join("\n") }
        } else {
            "\u{2014}".to_string()
        }
    } else {
        "\u{2014}".to_string()
    }
}

fn section_label(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .css_classes(["section-title"])
        .halign(gtk::Align::Start)
        .margin_top(8)
        .build()
}

fn bold_label(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .halign(gtk::Align::Start)
        .build()
}

fn status_icon(texture: &gdk::Texture, class: &str) -> gtk::Box {
    let picture = gtk::Picture::for_paintable(texture);
    picture.set_can_shrink(true);
    picture.set_size_request(48, 48);
    let boxx = gtk::Box::new(gtk::Orientation::Vertical, 0);
    boxx.add_css_class(class);
    boxx.add_css_class("icon-frame");
    boxx.append(&picture);
    boxx
}

#[cfg(feature = "gtk4")]
fn icon_texture(path: &str) -> gdk::Texture {
    gdk::Texture::from_filename(path)
        .unwrap_or_else(|_| gdk::Texture::from_filename("assets/c2pa.jpg")
            .unwrap_or_else(|_| {
                eprintln!("Warning: no icon assets found, using blank texture");
                let bytes = glib::Bytes::from_owned(vec![128u8; 48 * 48 * 4]);
                gdk::MemoryTexture::new(48, 48, gdk::MemoryFormat::B8g8r8a8, &bytes, 48 * 4).upcast()
            }))
}
