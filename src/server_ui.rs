use gtk::prelude::*;
use std::sync::{Arc, Mutex};
use crate::network;

pub fn create_server_window(
    app: &gtk::Application,
    payloader: Arc<Mutex<Option<gst::Element>>>,
    manifest_state: &Arc<Mutex<bool>>,
    fake_title: &str,
) -> gtk::ApplicationWindow {
    let css = gtk::CssProvider::new();
    css.load_from_data(
        ".section-title { font-size: 14px; font-weight: bold; margin-top: 12px; } \
         .data-label   { font-size: 13px; margin: 2px 0; } \
         .switch-row   { margin: 8px 0; }",
    );
    gtk::style_context_add_provider_for_display(
        &gdk::Display::default().expect("No display"),
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let window = gtk::ApplicationWindow::new(app);
    window.set_title(Some("DSC Server — Tampering Controls"));
    window.set_default_size(440, 380);

    let container = gtk::Box::new(gtk::Orientation::Vertical, 8);
    container.set_margin_start(16);
    container.set_margin_end(16);
    container.set_margin_top(12);
    container.set_margin_bottom(12);

    let title = gtk::Label::builder()
        .label("DSC Server — Tampering Controls")
        .css_classes(["section-title"])
        .halign(gtk::Align::Center)
        .build();
    container.append(&title);

    // Bitstream tamper
    let bt_label = gtk::Label::builder()
        .label("Bitstream Integrity")
        .css_classes(["data-label"])
        .halign(gtk::Align::Start)
        .build();
    let tamper_switch = gtk::Switch::builder().active(false).valign(gtk::Align::Center).build();
    let tamper_status = gtk::Label::builder()
        .label("CLEAN")
        .css_classes(["data-label"])
        .halign(gtk::Align::Start)
        .build();

    let bt_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    bt_row.add_css_class("switch-row");
    bt_row.append(&bt_label);
    bt_row.append(&tamper_switch);
    bt_row.append(&tamper_status);
    container.append(&bt_row);

    // Manifest tamper
    let mt_label = gtk::Label::builder()
        .label("Manifest Authenticity")
        .css_classes(["data-label"])
        .halign(gtk::Align::Start)
        .build();
    let manifest_switch = gtk::Switch::builder().active(false).valign(gtk::Align::Center).build();
    let manifest_status = gtk::Label::builder()
        .label("ORIGINAL")
        .css_classes(["data-label"])
        .halign(gtk::Align::Start)
        .build();

    let mt_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    mt_row.add_css_class("switch-row");
    mt_row.append(&mt_label);
    mt_row.append(&manifest_switch);
    mt_row.append(&manifest_status);
    container.append(&mt_row);

    // Stream info
    let sec_info = gtk::Label::builder()
        .label("Stream Info")
        .css_classes(["section-title"])
        .halign(gtk::Align::Start)
        .margin_top(16)
        .build();
    let local_ip = network::local_ip();
    let stream_info = gtk::Label::builder()
        .label(&format!(
            "Server IP: {}\nWHIP: http://{}:8190\nWHEP: http://{}:8191\nManifest HTTP: http://{}:8765",
            local_ip, local_ip, local_ip, local_ip
        ))
        .css_classes(["data-label"])
        .halign(gtk::Align::Start)
        .build();
    container.append(&sec_info);
    container.append(&stream_info);

    window.set_child(Some(&container));

    let p = payloader.clone();
    let ts = tamper_status.clone();
    tamper_switch.connect_state_set(move |_sw, state| {
        let payloader_guard = p.lock().unwrap();
        if let Some(ref pay) = *payloader_guard {
            if state {
                pay.set_property("config-interval", -1i32);
                ts.set_label("TAMPERED");
                eprintln!("\n>>> Tamper ON (switch)");
            } else {
                pay.set_property("config-interval", 0i32);
                ts.set_label("CLEAN");
                eprintln!("\n>>> Tamper OFF (switch)");
            }
            glib::Propagation::Proceed
        } else {
            eprintln!("\n>>> No payloader available yet (wait for WHIP connection)");
            glib::Propagation::Stop
        }
    });

    let m = manifest_state.clone();
    let ms = manifest_status.clone();
    let ft = fake_title.to_string();
    manifest_switch.connect_state_set(move |_sw, state| {
        let mut guard = m.lock().unwrap();
        *guard = !state;
        if state {
            ms.set_label(&format!("TAMPERED ({})", ft));
            eprintln!("\n>>> Manifest TAMPERED (switch)");
        } else {
            ms.set_label("ORIGINAL");
            eprintln!("\n>>> Manifest ORIGINAL (switch)");
        }
        glib::Propagation::Proceed
    });

    window
}
