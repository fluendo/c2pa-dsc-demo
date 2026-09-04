use gtk::prelude::*;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use crate::network;
use crate::server::{start_tamper_cycle, stop_tamper, TamperControl};

pub fn create_server_window(
    app: &gtk::Application,
    control: Arc<TamperControl>,
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

    // Peers
    let sec_peers = gtk::Label::builder()
        .label("Peers")
        .css_classes(["section-title"])
        .halign(gtk::Align::Start)
        .margin_top(16)
        .build();
    let peers_info = gtk::Label::builder()
        .label("Source (WHIP): waiting\nPlayer (WHEP): waiting\nVideo: waiting")
        .css_classes(["data-label"])
        .halign(gtk::Align::Start)
        .build();
    container.append(&sec_peers);
    container.append(&peers_info);

    window.set_child(Some(&container));

    {
        let peers_info = peers_info.clone();
        let source_connected = control.source_connected.clone();
        let player_connected = control.player_connected.clone();
        let last_video = control.last_video.clone();
        glib::timeout_add_local(Duration::from_millis(500), move || {
            let source = if source_connected.load(Ordering::SeqCst) {
                "connected"
            } else {
                "waiting"
            };
            let player = if player_connected.load(Ordering::SeqCst) {
                "connected"
            } else {
                "waiting"
            };
            let flowing = last_video
                .lock()
                .unwrap()
                .map(|t| t.elapsed() < Duration::from_secs(2))
                .unwrap_or(false);
            let video = if source == "connected" && player == "connected" && flowing {
                "flowing (source → player)"
            } else if flowing {
                "flowing"
            } else {
                "no video"
            };
            peers_info.set_label(&format!(
                "Source (WHIP): {}\nPlayer (WHEP): {}\nVideo: {}",
                source, player, video
            ));
            glib::ControlFlow::Continue
        });
    }

    let control = control.clone();
    let ts = tamper_status.clone();
    tamper_switch.connect_state_set(move |_sw, state| {
        if state {
            start_tamper_cycle(&control);
            ts.set_label("TAMPERED");
            eprintln!("\n>>> Tamper ON (sustained — switch off to restore)");
        } else {
            stop_tamper(&control);
            ts.set_label("CLEAN");
            eprintln!("\n>>> Tamper OFF (clean stream)");
        }
        glib::Propagation::Proceed
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
