mod cert;
mod server;
mod source;
mod player;
mod manifest;
mod player_ui;
mod server_ui;
mod source_ui;
mod network;

use anyhow::Result;
use cert::DscConfig;
use clap::Parser;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Parser, Debug)]
#[command(name = "c2pa-dsc-live-demo")]
struct Args {
    #[arg(long, default_value = "http://127.0.0.1:8190")]
    whip_server: String,

    #[arg(long, default_value = "http://127.0.0.1:8191")]
    whep_server: String,

    #[arg(long)]
    server_only: bool,

    #[arg(long)]
    source_only: bool,

    #[arg(long)]
    player_only: bool,

    #[arg(long)]
    demo_untrusted_signer: bool,

    #[arg(long)]
    demo_ai_filter: bool,

    #[arg(long, default_value = "1")]
    ai_effect: u32,

    #[arg(long, default_value = "95")]
    ai_effect_intensity: f32,

    #[arg(long, default_value = "/opt/fluendo/fluanonymizer/shared/raven")]
    ai_model_path: String,

    #[arg(long)]
    software_encoder: bool,

    #[arg(long, default_value = "FakeStream")]
    demo_manifest_title: String,

    #[arg(long, default_value = "127.0.0.1:8765")]
    manifest_host: String,

    #[arg(long, default_value = "/dev/video0")]
    camera_device: String,

    #[arg(long, default_value = "/tmp/c2pa-certs")]
    certs_dir: PathBuf,

    #[arg(long, default_value = "config/c2pa_dsc_unified.cnf")]
    openssl_config: PathBuf,

    #[arg(long, default_value = "3")]
    substream_length: u32,

    #[arg(long, default_value = "sha256")]
    hash_method: String,

    #[arg(long)]
    content_uuid: Option<String>,
}

fn main() -> Result<()> {
    gst::init()?;
    std::fs::create_dir_all("dot")?;
    std::env::set_var("GST_DEBUG_DUMP_DOT_DIR", "dot");

    let args = Args::parse();
    let running = Arc::new(AtomicBool::new(true));

    ctrlc::set_handler({
        let r = running.clone();
        move || {
            eprintln!("\nShutting down...");
            r.store(false, Ordering::SeqCst);
        }
    })?;

    let whip_endpoint = format!("{}/whip/endpoint", args.whip_server);
    let whep_endpoint = format!("{}/whep/endpoint", args.whep_server);

    let cert_paths = if args.demo_untrusted_signer && !args.player_only {
        let paths = cert::CertPaths::impersonator(&args.certs_dir);
        cert::ensure_impersonator_certs(&paths)?;
        println!(">>> UNTRUSTED SIGNER MODE: using impersonator cert (NOT in CA chain)");
        paths
    } else {
        cert::CertPaths::new(&args.certs_dir)
    };
    // Only the source/signature side needs generated keys. The player only
    // needs the trust anchor (ca.crt), which is copied from the source machine;
    // generating a new PKI here would overwrite that copied ca.crt.
    if !args.player_only {
        cert::ensure_certs(&cert_paths, &args.openssl_config)?;
    }

    let dsc_config = DscConfig {
        private_key_path: cert_paths.private_key.clone(),
        cert_path: cert_paths.cert.clone(),
        trust_store_path: cert_paths.trust_store.clone(),
        key_store_dir: args.certs_dir.clone(),
        substream_length: args.substream_length,
        hash_method: args.hash_method.clone(),
        content_uuid: args.content_uuid.clone().or_else(|| {
            Some(uuid::Uuid::new_v4().simple().to_string())
        }),
        camera_device: Some(args.camera_device.clone()),
        manifest_uri_template: None,
        public_key_uri: None,
        demo_ai_filter: args.demo_ai_filter,
        ai_effect: args.ai_effect,
        ai_effect_intensity: args.ai_effect_intensity,
        ai_model_path: args.ai_model_path.clone(),
        software_encoder: args.software_encoder,
    };

    if args.demo_ai_filter {
        match gst::ElementFactory::find("flufaceanonymizer") {
            None => {
                eprintln!(
                    "ERROR: --demo-ai-filter requires the flufaceanonymizer GStreamer element,\n\
                     but it was not found. Install the Fluendo anonymizer package and use the\n\
                     launcher script (scripts/run-ai-filter.sh) which sets up the required\n\
                     LD_LIBRARY_PATH/GST_PLUGIN_PATH environment."
                );
                std::process::exit(1);
            }
            Some(_) => {
                println!(">>> AI FILTER: flufaceanonymizer element detected");
            }
        }
    }

    if args.source_only {
        return run_source_only(&args, whip_endpoint, dsc_config, running);
    }

    if args.server_only {
        return run_server_only(&args, running);
    }

    if args.player_only {
        return run_player_only(&args, whep_endpoint, dsc_config);
    }

    run_full(args, whip_endpoint, whep_endpoint, dsc_config, running)
}

fn run_source_only(
    args: &Args, whip_endpoint: String, mut dsc_config: DscConfig, running: Arc<AtomicBool>,
) -> Result<()> {
    dsc_config.manifest_uri_template = Some(format!(
        "http://{}/dsc-c2pa-{{uuid}}.c2pa", args.manifest_host,
    ));
    dsc_config.public_key_uri = Some(format!(
        "http://{}/certs/{}",
        args.manifest_host,
        dsc_config.cert_path.file_name().unwrap().to_string_lossy(),
    ));
    use gtk::prelude::*;
    let cam_dev = args.camera_device.clone();
    let app = gtk::Application::new(Some("com.fluendo.c2pa-dsc.source"), Default::default());
    let whip = whip_endpoint.clone();
    let dsc = dsc_config.clone();
    let r = running.clone();
    let controls = Arc::new(Mutex::new(source::SourceControls::new()));
    {
        let mut c = controls.lock().unwrap();
        c.available = gst::ElementFactory::find("flufaceanonymizer").is_some();
        c.enabled = dsc_config.demo_ai_filter && c.available;
        c.effect = dsc_config.ai_effect;
        c.effect_intensity = dsc_config.ai_effect_intensity;
    }
    let controls_gtk = controls.clone();
    app.connect_activate(move |app| {
        let _sw = source_ui::create_source_window(app, &cam_dev, controls_gtk.clone());
        _sw.present();
    });
    std::thread::spawn(move || {
        let _ = source::run(&whip, &dsc, r, controls);
    });
    app.run_with_args(&[] as &[&str]);
    Ok(())
}

fn run_server_only(args: &Args, running: Arc<AtomicBool>) -> Result<()> {
    use gtk::prelude::*;
    let control = Arc::new(server::TamperControl::new());
    let fake_title = args.demo_manifest_title.clone();
    // Generate fake manifest for the manifest swap demo
    let fake_data = match manifest::generate_fake_manifest(&fake_title) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Warning: could not generate fake manifest: {}", e);
            Vec::new()
        }
    };
    let ms_http = manifest::start_server(
        8765,
        Arc::new(Mutex::new(fake_data)),
        args.certs_dir.clone(),
    );
    let ms = ms_http.clone();
    let whip = args.whip_server.clone();
    let whep = args.whep_server.clone();
    let running_clone = running.clone();
    let control_server = control.clone();
    let app = gtk::Application::new(Some("com.fluendo.c2pa-dsc.server"), Default::default());
    let ms2 = ms.clone();
    let control_gtk = control.clone();
    app.connect_activate(move |app| {
        let _sw = server_ui::create_server_window(app, control_gtk.clone(), &ms2, &fake_title);
        _sw.present();
    });
    std::thread::spawn(move || {
        let _ = server::run(whip, whep, running_clone, control_server);
    });
    app.run_with_args(&[] as &[&str]);
    Ok(())
}

fn run_player_only(_args: &Args, whep_endpoint: String, dsc_config: DscConfig) -> Result<()> {
    use gtk::prelude::*;
    println!("Waiting for WHEP server at {}...", whep_endpoint);
    player::wait_for_server(&whep_endpoint);
    let whep = whep_endpoint.clone();
    let dsc_gtk = dsc_config.clone();
    let app = gtk::Application::new(Some("com.fluendo.c2pa-dsc.player"), Default::default());
    app.connect_activate(move |app| {
        if let Err(e) = player::run_gtk(app, &whep, &dsc_gtk) {
            eprintln!("Player GTK error: {}", e);
        }
    });
    app.run_with_args(&[] as &[&str]);
    Ok(())
}

fn run_full(
    args: Args, whip_endpoint: String, whep_endpoint: String,
    mut dsc_config: DscConfig, running: Arc<AtomicBool>,
) -> Result<()> {
    // Clean old manifest files
    if let Ok(entries) = std::fs::read_dir("/tmp") {
        for e in entries.flatten() {
            if e.file_name().to_string_lossy().starts_with("dsc-c2pa-") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }

    let fake_manifest = match manifest::generate_fake_manifest(&args.demo_manifest_title) {
        Ok(data) => Arc::new(Mutex::new(data)),
        Err(e) => {
            eprintln!("Warning: could not generate fake manifest: {}", e);
            Arc::new(Mutex::new(Vec::new()))
        }
    };
    let manifest_http_state = manifest::start_server(
        8765,
        fake_manifest,
        args.certs_dir.clone(),
    );
    dsc_config.manifest_uri_template = Some(format!(
        "http://{}/dsc-c2pa-{{uuid}}.c2pa", args.manifest_host,
    ));
    dsc_config.public_key_uri = Some(format!(
        "http://{}/certs/{}",
        args.manifest_host,
        dsc_config.cert_path.file_name().unwrap().to_string_lossy(),
    ));

    // Server thread
    let server_running = running.clone();
    let server_whip = args.whip_server.clone();
    let server_whep = args.whep_server.clone();
    let control = Arc::new(server::TamperControl::new());
    let control_server = control.clone();
    std::thread::spawn(move || {
        if let Err(e) = server::run(server_whip, server_whep, server_running, control_server) {
            eprintln!("Server thread error: {}", e);
        }
    });

    // Stdin toggle thread
    let control_stdin = control.clone();
    let ms = manifest_http_state.clone();
    std::thread::spawn(move || {
        let mut buf = String::new();
        println!("  Commands: t = toggle bitstream tamper, m = toggle manifest swap, q = quit");
        loop {
            buf.clear();
            if std::io::stdin().read_line(&mut buf).is_ok() {
                match buf.trim() {
                    "t" | "T" => {
                        let enabled = server::toggle_tamper(&control_stdin);
                        if enabled {
                            eprintln!("\n>>> Tamper ON (sustained — switch off to restore)");
                        } else {
                            eprintln!("\n>>> Tamper OFF (clean stream, DSC will pass)");
                        }
                    }
                    "m" | "M" => {
                        let mut guard = ms.lock().unwrap();
                        *guard = !*guard;
                        if *guard {
                            eprintln!("\n>>> Manifest TAMPERED (provenance forged, C2PA will fail)");
                        } else {
                            eprintln!("\n>>> Manifest ORIGINAL restored (authentic provenance)");
                        }
                    }
                    "q" | "Q" => { eprintln!("\n>>> Quitting..."); break; }
                    "" => {}
                    _ => eprintln!("  Commands: t = toggle bitstream tamper, m = toggle manifest swap, q = quit"),
                }
            }
        }
    });

    std::thread::sleep(std::time::Duration::from_secs(2));

    // Source thread
    let dsc_source = dsc_config.clone();
    let source_running = running.clone();
    let controls = Arc::new(Mutex::new(source::SourceControls::new()));
    {
        let mut c = controls.lock().unwrap();
        c.available = gst::ElementFactory::find("flufaceanonymizer").is_some();
        c.enabled = dsc_config.demo_ai_filter && c.available;
        c.effect = dsc_config.ai_effect;
        c.effect_intensity = dsc_config.ai_effect_intensity;
    }
    let controls_gtk = controls.clone();
    std::thread::spawn(move || {
        if let Err(e) = source::run(&whip_endpoint, &dsc_source, source_running, controls) {
            eprintln!("Source thread error: {}", e);
        }
    });

    std::thread::sleep(std::time::Duration::from_secs(2));

    println!("Waiting for WHEP server...");
    player::wait_for_server(&whep_endpoint);

    // GTK4 windows (always)
    use gtk::prelude::*;
    let whep = whep_endpoint.clone();
    let dsc_gtk = dsc_config.clone();
    let control_gtk = control.clone();
    let manifest_gtk = manifest_http_state.clone();
    let fake_title = args.demo_manifest_title.clone();
    let cam_dev = args.camera_device.clone();
    let app = gtk::Application::new(Some("com.fluendo.c2pa-dsc-demo"), Default::default());
    app.connect_activate(move |app| {
        if let Err(e) = player::run_gtk(app, &whep, &dsc_gtk) {
            eprintln!("Player GTK error: {}", e);
        }
        let _sw = server_ui::create_server_window(app, control_gtk.clone(), &manifest_gtk, &fake_title);
        _sw.present();
        let _sw = source_ui::create_source_window(app, &cam_dev, controls_gtk.clone());
        _sw.present();
    });
    app.run_with_args(&[] as &[&str]);
    Ok(())
}
