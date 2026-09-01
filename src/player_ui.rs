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
    pub ai_modified: bool,
}

pub struct PlayerUi {
    pub container: gtk::Box,
    video_box: gtk::Box,
    dsc_icon: gtk::Box,
    c2pa_icon: gtk::Box,
    dsc_status: gtk::Label,
    c2pa_status: gtk::Label,
    ai_row: gtk::Box,
    ai_status: gtk::Label,
    title: gtk::Label,
    signed_by: gtk::Label,
    manifest_tree: gtk::Label,
}

impl PlayerUi {
    pub fn new(window_width: i32, _window_height: i32, whep_endpoint: &str) -> Self {
        // CSS
        let css = gtk::CssProvider::new();
        css.load_from_data(
            ".status-wait { background: #888888; border-radius: 3px; } \
             .status-ok    { background: #00aa00; border-radius: 3px; } \
             .status-fail  { background: #cc0000; border-radius: 3px; } \
             .status-ai    { background: #d97706; border-radius: 3px; } \
             .icon-frame   { padding: 4px; margin: 4px; } \
             .panel        { background: #ffffff; border: 1px solid #e0e0e0; border-radius: 12px; padding: 12px; } \
             .header       { background: #000000; padding: 8px 16px; } \
             .footer       { background: #000000; } \
             .section-title { font-size: 14px; font-weight: bold; margin-top: 12px; } \
             .data-label   { font-size: 13px; font-family: monospace; margin: 2px 0; } \
             .demo-desc    { font-size: 14px; color: #555555; margin-bottom: 8px; } \
             .ai-text      { color: #ffffff; font-weight: bold; font-size: 12px; } \
             .footer-text  { font-size: 11px; color: #cccccc; } \
             .header-title { font-size: 18px; font-weight: bold; color: #ffffff; }",
        );
        gtk::style_context_add_provider_for_display(
            &gdk::Display::default().expect("No display"),
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        // Header
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        header.set_halign(gtk::Align::Fill);
        header.add_css_class("header");

        // Left spacer (centers the header content)
        let left_spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        left_spacer.set_hexpand(true);
        header.append(&left_spacer);

        // Fraunhofer HHI logo (left of the Fluendo logo)
        if let Ok(hhi_texture) = gdk::Texture::from_filename("assets/fraunhofer_hhi_logo.png") {
            let hhi_logo = gtk::Picture::for_paintable(&hhi_texture);
            hhi_logo.set_can_shrink(true);
            hhi_logo.set_size_request(104, 22);
            header.append(&hhi_logo);
        }

        if let Ok(texture) = gdk::Texture::from_filename("assets/logo-fluendo.png") {
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

        // Right spacer (centers the header content)
        let right_spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        right_spacer.set_hexpand(true);
        header.append(&right_spacer);

        // Footer
        let footer_text = gtk::Label::builder()
            .label("© 2026 Fraunhofer HHI & Fluendo S.A.  |  Powered by GStreamer")
            .css_classes(["footer-text"])
            .halign(gtk::Align::Center)
            .margin_top(8)
            .margin_bottom(8)
            .build();
        let footer = gtk::Box::new(gtk::Orientation::Vertical, 0);
        footer.add_css_class("footer");
        footer.set_halign(gtk::Align::Fill);
        footer.append(&footer_text);

        // Diagram area (left of the video)
        let diagram_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        diagram_box.add_css_class("panel");
        diagram_box.set_size_request(360, -1);
        diagram_box.set_hexpand(false);
        diagram_box.set_vexpand(true);
        diagram_box.set_valign(gtk::Align::Fill);
        diagram_box.set_margin_start(12);
        diagram_box.set_margin_end(8);

        let top_spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
        top_spacer.set_vexpand(true);
        let bottom_spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
        bottom_spacer.set_vexpand(true);

        let demo_desc = gtk::Label::builder()
            .label("Live C2PA + DSC provenance demo: video is signed at the camera, streamed over WebRTC, and verified in real time.")
            .css_classes(["demo-desc"])
            .halign(gtk::Align::Center)
            .hexpand(true)
            .justify(gtk::Justification::Center)
            .xalign(0.5)
            .wrap(true)
            .build();

        diagram_box.append(&top_spacer);
        diagram_box.append(&demo_desc);

        if let Ok(texture) = gdk::Texture::from_filename("assets/architecture-overview.svg") {
            let diagram = gtk::Picture::for_paintable(&texture);
            diagram.set_can_shrink(true);
            diagram.set_hexpand(true);
            diagram.set_size_request(-1, 370);
            diagram_box.append(&diagram);
        }
        diagram_box.append(&bottom_spacer);

        // Video area (left)
        let video_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        video_box.add_css_class("panel");
        video_box.set_size_request(window_width * 45 / 100, -1);
        video_box.set_hexpand(false);
        video_box.set_vexpand(true);
        video_box.set_valign(gtk::Align::Fill);

        let video_placeholder = gtk::Label::builder()
            .label("Video will appear here\n(ximagesink or gtk4paintablesink)")
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .vexpand(true)
            .build();
        video_box.append(&video_placeholder);

        // Right info panel
        let right_panel = gtk::Box::new(gtk::Orientation::Vertical, 8);
        right_panel.add_css_class("panel");
        right_panel.set_margin_start(12);
        right_panel.set_margin_end(16);
        right_panel.set_vexpand(true);
        right_panel.set_hexpand(true);
        right_panel.set_valign(gtk::Align::Fill);

        // Validation Status section
        let sec_status = section_label("Validation Status");

        let dsc_texture = icon_texture("assets/dsc.png");
        let c2pa_texture = icon_texture("assets/c2pa.jpg");

        let dsc_icon = status_icon(&dsc_texture, "status-wait");
        let c2pa_icon = status_icon(&c2pa_texture, "status-wait");
        let ai_icon = ai_icon();

        let dsc_status = gtk::Label::builder().label("").css_classes(["data-label"]).halign(gtk::Align::Start).build();
        let c2pa_status = gtk::Label::builder().label("").css_classes(["data-label"]).halign(gtk::Align::Start).build();
        let ai_status = gtk::Label::builder().label("AI modification detected").css_classes(["data-label"]).halign(gtk::Align::Start).build();

        let dsc_row = status_row(&dsc_icon, &dsc_status);
        let c2pa_row = status_row(&c2pa_icon, &c2pa_status);
        let ai_row = status_row(&ai_icon, &ai_status);
        ai_row.set_visible(false);

        // C2PA data section
        let sec_manifest = section_label("C2PA data");
        let title = gtk::Label::builder()
            .label("Title: —")
            .css_classes(["data-label"])
            .halign(gtk::Align::Start)
            .build();
        let source = gtk::Label::builder()
            .label("Source: Webcam")
            .css_classes(["data-label"])
            .halign(gtk::Align::Start)
            .build();
        let signed_by = gtk::Label::builder()
            .label("Signed-by: —")
            .css_classes(["data-label"])
            .halign(gtk::Align::Start)
            .build();

        // C2PA Manifest list, highlighted in its own rounded box
        let manifest_box_label = section_label("C2PA Manifest");
        let manifest_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
        manifest_box.add_css_class("panel");
        let manifest_tree = gtk::Label::builder()
            .label("Waiting for manifest…")
            .css_classes(["data-label"])
            .halign(gtk::Align::Start)
            .build();
        manifest_box.append(&manifest_tree);

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

        // Assemble right panel, vertically centered with expanding spacers
        let top_spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
        top_spacer.set_vexpand(true);
        let bottom_spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
        bottom_spacer.set_vexpand(true);

        right_panel.append(&top_spacer);
        right_panel.append(&sec_status);
        right_panel.append(&dsc_row);
        right_panel.append(&c2pa_row);
        right_panel.append(&ai_row);
        right_panel.append(&sec_manifest);
        right_panel.append(&title);
        right_panel.append(&source);
        right_panel.append(&signed_by);
        right_panel.append(&manifest_box_label);
        right_panel.append(&manifest_box);
        right_panel.append(&sec_info);
        right_panel.append(&info_text);
        right_panel.append(&bottom_spacer);

        // Main layout
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        content.set_vexpand(true);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.append(&diagram_box);
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
            ai_row,
            ai_status,
            title,
            signed_by,
            manifest_tree,
        }
    }

    pub fn set_video_widget(&self, widget: &impl gtk::prelude::IsA<gtk::Widget>) {
        while let Some(child) = self.video_box.first_child() {
            self.video_box.remove(&child);
        }
        self.video_box.append(widget);
    }

    pub fn update(&self, result: &DscVerificationResult) {
        // DSC verification row
        let dsc_class = match result.dsc_status.as_str() {
            "valid" => "status-ok",
            "invalid" => "status-fail",
            _ => "status-wait",
        };
        self.dsc_icon.set_css_classes(&[dsc_class, "icon-frame"]);
        let dsc_text = status_text(&result.dsc_status);
        self.dsc_status.set_markup(&format!("<span size='small'>DSC: <tt>{}</tt></span>", dsc_text));

        // C2PA verification row
        let c2pa_class = if result.c2pa_status == "valid" {
            "status-ok"
        } else if result.c2pa_status.is_empty() || result.c2pa_status == "unknown" {
            "status-wait"
        } else {
            "status-fail"
        };
        self.c2pa_icon.set_css_classes(&[c2pa_class, "icon-frame"]);
        let c2pa_text = status_text(&result.c2pa_status);
        self.c2pa_status.set_markup(&format!("<span size='small'>C2PA: <tt>{}</tt></span>", c2pa_text));

        // AI modification row (only when the c2pa.edited assertion is present)
        self.ai_row.set_visible(result.ai_modified);
        self.ai_status.set_markup("<span size='small' foreground='#d97706'>AI modification detected</span>");

        // Claim title and signer (top-level C2PA data)
        if result.c2pa_status == "valid" {
            let t = if result.manifest_title.is_empty() { "—" } else { &result.manifest_title };
            self.title.set_label(&format!("Title: {}", t));
            let g = if result.claim_generator.is_empty() { "—" } else { &result.claim_generator };
            self.signed_by.set_label(&format!("Signed-by: {}", g));
        } else {
            self.title.set_label("Title: —");
            self.signed_by.set_label("Signed-by: —");
        }

        self.manifest_tree.set_label(&format_manifest_tree(result));
    }
}

fn status_text(status: &str) -> &str {
    match status {
        "valid" => "Verified",
        "invalid" => "Invalid",
        "" | "unknown" => "Waiting…",
        other => other,
    }
}

fn format_manifest_tree(result: &DscVerificationResult) -> String {
    if result.c2pa_status == "valid" {
        let mut lines = Vec::new();
        lines.push("Claim".to_string());
        let generator = if result.claim_generator.is_empty() {
            "—"
        } else {
            &result.claim_generator
        };
        lines.push(format!("  claim generator: {}", generator));
        lines.push("Assertions".to_string());
        lines.push("  c2pa.actions".to_string());
        let actions = actions_names(&result.actions);
        if actions.is_empty() {
            lines.push("    —".to_string());
        } else {
            for action in actions {
                lines.push(format!("    {}", action));
            }
        }
        lines.push("  c2pa.ingredient".to_string());
        let ingredients = ingredient_names(&result.provenance);
        if ingredients.is_empty() {
            lines.push("    —".to_string());
        } else {
            for ingredient in ingredients {
                lines.push(format!("    {}", ingredient));
            }
        }
        lines.join("\n")
    } else if result.c2pa_status.is_empty() || result.c2pa_status == "unknown" {
        "Waiting for manifest…".to_string()
    } else {
        "Manifest unverified".to_string()
    }
}

fn actions_names(json: &str) -> Vec<String> {
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(json) {
        if let Some(actions) = val.get("actions").and_then(|a| a.as_array()) {
            return actions
                .iter()
                .filter_map(|a| a.get("action").and_then(|s| s.as_str()).map(|s| s.to_string()))
                .collect();
        }
    }
    Vec::new()
}

fn ingredient_names(json: &str) -> Vec<String> {
    if let Ok(vals) = serde_json::from_str::<Vec<serde_json::Value>>(json) {
        return vals
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
    }
    Vec::new()
}

fn section_label(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .css_classes(["section-title"])
        .halign(gtk::Align::Start)
        .margin_top(8)
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

fn status_row(icon: &gtk::Box, text: &gtk::Label) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.set_valign(gtk::Align::Center);
    row.append(icon);
    row.append(text);
    row
}

fn ai_icon() -> gtk::Box {
    let boxx = gtk::Box::new(gtk::Orientation::Vertical, 0);
    boxx.add_css_class("status-ai");
    boxx.add_css_class("icon-frame");
    boxx.set_valign(gtk::Align::Center);
    let label = gtk::Label::builder()
        .label("AI")
        .css_classes(["ai-text"])
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();
    boxx.append(&label);
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
