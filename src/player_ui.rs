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
    pub signer: String,
    pub digital_source_type: String,
    pub ai_modified: bool,
}

pub struct PlayerUi {
    pub container: gtk::Box,
    video_box: gtk::Box,
    dsc_box: gtk::Box,
    dsc_status: gtk::Label,
    c2pa_box: gtk::Box,
    c2pa_status: gtk::Label,
    ai_status: gtk::Box,
    title: gtk::Label,
    source: gtk::Label,
    signed_by: gtk::Label,
}

impl PlayerUi {
    pub fn new(window_width: i32, _window_height: i32, whep_endpoint: &str) -> Self {
        // CSS
        let css = gtk::CssProvider::new();
        css.load_from_data(
            ".status-wait { background: #888888; } \
             .status-ok    { background: #00aa00; } \
             .status-fail  { background: #cc0000; } \
             .status-ai    { background: #d97706; } \
             .status-box   { border-radius: 8px; padding: 8px 12px; } \
             .status-text  { color: #ffffff; font-weight: bold; font-size: 13px; } \
             .panel        { background: #ffffff; border: 1px solid #e0e0e0; border-radius: 12px; padding: 12px; } \
             .header       { background: #000000; padding: 8px 16px; } \
             .footer       { background: #000000; } \
             .section-title { font-size: 14px; font-weight: bold; margin-top: 12px; } \
             .diagram-title { font-size: 20px; font-weight: bold; margin-bottom: 8px; } \
             .data-label   { font-size: 13px; font-family: monospace; margin: 2px 0; } \
             .demo-desc    { font-size: 14px; color: #555555; margin-bottom: 8px; } \
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

        let diagram_title = gtk::Label::builder()
            .label("Architecture")
            .css_classes(["diagram-title"])
            .halign(gtk::Align::Center)
            .build();

        let demo_desc = gtk::Label::builder()
            .label("Live C2PA + DSC provenance demo: the camera feed is signed at the source, published over WHIP, relayed by the server, and delivered to this player over WHEP with real-time verification.")
            .css_classes(["demo-desc"])
            .halign(gtk::Align::Center)
            .hexpand(true)
            .justify(gtk::Justification::Center)
            .xalign(0.5)
            .wrap(true)
            .build();

        diagram_box.append(&top_spacer);
        diagram_box.append(&diagram_title);
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

        // DSC section
        let sec_dsc = section_label("Digitally Signed Content");
        let dsc_texture = icon_texture("assets/dsc.png");
        let dsc_status = gtk::Label::builder()
            .label("DSC: Waiting…")
            .css_classes(["status-text"])
            .halign(gtk::Align::Start)
            .build();
        let dsc_box = status_box(&dsc_texture, "status-wait", &dsc_status);

        // C2PA section
        let sec_c2pa = section_label("C2PA");
        let c2pa_texture = icon_texture("assets/c2pa.jpg");
        let c2pa_status = gtk::Label::builder()
            .label("C2PA: Waiting…")
            .css_classes(["status-text"])
            .halign(gtk::Align::Start)
            .build();
        let c2pa_box = status_box(&c2pa_texture, "status-wait", &c2pa_status);

        let ai_status_text = gtk::Label::builder()
            .label("AI modification detected")
            .css_classes(["status-text"])
            .halign(gtk::Align::Start)
            .build();
        let ai_status = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        ai_status.add_css_class("status-ai");
        ai_status.add_css_class("status-box");
        ai_status.append(&ai_status_text);
        ai_status.set_visible(false);

        let title = data_label("Title: —");
        let source = data_label("Source: —");
        let signed_by = data_label("Signed-by: —");

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
        right_panel.append(&sec_dsc);
        right_panel.append(&dsc_box);
        right_panel.append(&sec_c2pa);
        right_panel.append(&c2pa_box);
        right_panel.append(&ai_status);
        right_panel.append(&title);
        right_panel.append(&source);
        right_panel.append(&signed_by);
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
            dsc_box,
            dsc_status,
            c2pa_box,
            c2pa_status,
            ai_status,
            title,
            source,
            signed_by,
        }
    }

    pub fn set_video_widget(&self, widget: &impl gtk::prelude::IsA<gtk::Widget>) {
        while let Some(child) = self.video_box.first_child() {
            self.video_box.remove(&child);
        }
        self.video_box.append(widget);
    }

    pub fn update(&self, result: &DscVerificationResult) {
        // DSC status box
        let dsc_class = match result.dsc_status.as_str() {
            "valid" => "status-ok",
            "invalid" => "status-fail",
            _ => "status-wait",
        };
        self.dsc_box.set_css_classes(&[dsc_class, "status-box"]);
        self.dsc_status.set_label(&format!("DSC: {}", status_text(&result.dsc_status)));

        // C2PA status box
        let c2pa_class = if result.c2pa_status == "valid" {
            "status-ok"
        } else if result.c2pa_status.is_empty() || result.c2pa_status == "unknown" {
            "status-wait"
        } else {
            "status-fail"
        };
        self.c2pa_box.set_css_classes(&[c2pa_class, "status-box"]);
        self.c2pa_status.set_label(&format!("C2PA: {}", status_text(&result.c2pa_status)));

        // AI modification row (only when the c2pa.edited assertion is present)
        self.ai_status.set_visible(result.ai_modified);

        // C2PA data rows
        if result.c2pa_status == "valid" {
            let t = if result.manifest_title.is_empty() { "—" } else { &result.manifest_title };
            self.title.set_label(&format!("Title: {}", t));
            let src = ingredient_title(&result.provenance).unwrap_or_else(|| "—".to_string());
            self.source.set_label(&format!("Source: {}", src));
            let sig = if result.signer.is_empty() { "—" } else { &result.signer };
            self.signed_by.set_label(&format!("Signed-by: {}", sig));
        } else {
            self.title.set_label("Title: —");
            self.source.set_label("Source: —");
            self.signed_by.set_label("Signed-by: —");
        }
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

fn ingredient_title(provenance: &str) -> Option<String> {
    let vals: Vec<serde_json::Value> = serde_json::from_str(provenance).ok()?;
    vals.first()
        .and_then(|v| v.get("title"))
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
}

fn data_label(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .css_classes(["data-label"])
        .halign(gtk::Align::Start)
        .build()
}

fn section_label(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .css_classes(["section-title"])
        .halign(gtk::Align::Start)
        .margin_top(8)
        .build()
}

fn status_box(texture: &gdk::Texture, class: &str, text: &gtk::Label) -> gtk::Box {
    let picture = gtk::Picture::for_paintable(texture);
    picture.set_can_shrink(true);
    picture.set_size_request(24, 24);
    let boxx = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    boxx.add_css_class(class);
    boxx.add_css_class("status-box");
    boxx.append(&picture);
    boxx.append(text);
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
