use crate::cert::DscConfig;
use crate::player_ui::DscVerificationResult;
use crate::player_ui::PlayerUi;
use anyhow::Result;
use gst::prelude::*;
use std::sync::Arc;
use std::sync::Mutex;

fn make_element(factory: &str, name: &str) -> Result<gst::Element> {
    gst::ElementFactory::make(factory)
        .name(name)
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to create {factory} element: {e}"))
}

fn make_whep_src(whep_endpoint: &str) -> Result<gst::Element> {
    let src = gst::ElementFactory::make("whepclientsrc")
        .name("whepsrc")
        .build()?;
    src.dynamic_cast_ref::<gst::ChildProxy>()
        .ok_or_else(|| anyhow::anyhow!("whepclientsrc missing ChildProxy"))?
        .set_child_property("signaller::whep-endpoint", whep_endpoint);

    // This demo is H.265-only end to end (the DSC SEI only exists in H.265), and
    // the player decodes H.265. Constrain the SDP offer to H.265 so the WHEP
    // server payloads the relayed H.265 as-is instead of transcoding it to VP8.
    src.set_property("video-codecs", gst::Array::new(["H265"]));

    Ok(src)
}

pub fn wait_for_server(whep_endpoint: &str) -> bool {
    let client = reqwest::blocking::Client::new();
    for _ in 0..30 {
        match client
            .request(reqwest::Method::OPTIONS, whep_endpoint)
            .send()
        {
            Ok(resp) if resp.status().is_success() => {
                std::thread::sleep(std::time::Duration::from_secs(1));
                return true;
            }
            _ => {
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        }
    }
    false
}

fn extract_dsc_result(s: &gst::StructureRef) -> Option<DscVerificationResult> {
    if s.name().as_str() != "dsc-c2pa-verification-result" {
        return None;
    }
    let actions = s.get::<String>("c2pa-actions").unwrap_or_default();
    let digital_source_type = s.get::<String>("c2pa-digital-source-type").unwrap_or_default();
    let ai_modified =
        has_ai_edit(&actions) || digital_source_type.contains("trainedAlgorithmicMedia");
    Some(DscVerificationResult {
        dsc_status: s.get::<String>("dsc-status").unwrap_or_else(|_| "unknown".into()),
        c2pa_status: s.get::<String>("c2pa-status").unwrap_or_else(|_| "unknown".into()),
        manifest_title: s.get::<String>("c2pa-manifest-title").unwrap_or_default(),
        provenance: s.get::<String>("c2pa-provenance").unwrap_or_default(),
        actions,
        claim_generator: s.get::<String>("c2pa-claim-generator").unwrap_or_default(),
        signer: s.get::<String>("c2pa-signer").unwrap_or_default(),
        digital_source_type,
        ai_modified,
    })
}

fn has_ai_edit(actions: &str) -> bool {
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(actions) {
        if let Some(list) = val.get("actions").and_then(|a| a.as_array()) {
            return list
                .iter()
                .any(|a| a.get("action").and_then(|s| s.as_str()) == Some("c2pa.edited"));
        }
    }
    false
}

fn link_whep_pads(
    whepclientsrc: &gst::Element,
    videoconvert: &gst::Element,
    pipeline: &gst::Pipeline,
    dsc: &DscConfig,
) {
    let pipeline_weak = pipeline.downgrade();
    let vc_weak = videoconvert.downgrade();
    let dsc_trust_store = dsc.trust_store_path.clone();
    let dsc_key_store = dsc.key_store_dir.clone();

    whepclientsrc.connect_pad_added(move |_src, pad| {
        let Some(pipeline) = pipeline_weak.upgrade() else {
            return;
        };
        let pad_name = pad.name().to_string();
        eprintln!("  -> whepclientsrc pad_added: {}", pad_name);

        let q = gst::ElementFactory::make("queue")
            .name(format!("queue_{}", pad_name))
            .build()
            .unwrap();
        pipeline.add(&q).unwrap();
        q.sync_state_with_parent().unwrap();

        let qsink = q.static_pad("sink").unwrap();
        pad.link(&qsink).unwrap();

        if pad_name.contains("audio") {
            link_audio(&q, &pipeline, &pad_name);
        } else {
            let video_decoder = link_video_chain(
                &q, &pipeline, &pad_name,
                &dsc_trust_store, &dsc_key_store,
            );
            let Some(vc) = vc_weak.upgrade() else {
                return;
            };
            let dec_src = video_decoder.static_pad("src").unwrap();
            let vcsink = vc.static_pad("sink").unwrap();
            dec_src.link(&vcsink).unwrap();
            pipeline.debug_to_dot_file(gst::DebugGraphDetails::all(), "player-playing");
        }
    });
}

fn link_video_chain(
    q: &gst::Element,
    pipeline: &gst::Pipeline,
    pad_name: &str,
    trust_store_path: &std::path::Path,
    key_store_dir: &std::path::Path,
) -> gst::Element {
    let h265parse = gst::ElementFactory::make("h265parse")
        .name(format!("h265parse_{}", pad_name))
        .build()
        .unwrap();

    let decoder = gst::ElementFactory::make("avdec_h265")
        .name(format!("decoder_{}", pad_name))
        .build()
        .unwrap_or_else(|_| {
            gst::ElementFactory::make("vah265dec")
                .name(format!("decoder_{}", pad_name))
                .build()
                .unwrap_or_else(|_| {
                    panic!("No H.265 decoder available (tried avdec_h265, vah265dec)")
                })
        });

    pipeline.add(&h265parse).unwrap();
    h265parse.sync_state_with_parent().unwrap();

    let qsrc = q.static_pad("src").unwrap();
    let hp_sink = h265parse.static_pad("sink").unwrap();
    qsrc.link(&hp_sink).unwrap();

    let dsc_caps = gst::ElementFactory::make("capsfilter")
        .name(format!("dsc-caps_{}", pad_name))
        .property(
            "caps",
            gst::Caps::builder("video/x-h265")
                .field("stream-format", "hvc1")
                .field("alignment", "au")
                .build(),
        )
        .build()
        .unwrap();

    let dscverifier = gst::ElementFactory::make("dscverifier")
        .name(format!("dscverifier_{}", pad_name))
        .build()
        .unwrap();
    dscverifier.set_property("buffer", false);
    dscverifier.set_property_from_str(
        "trust-store-path",
        trust_store_path.to_string_lossy().as_ref(),
    );
    dscverifier.set_property_from_str(
        "key-store-path",
        key_store_dir.to_string_lossy().as_ref(),
    );

    pipeline.add(&dsc_caps).unwrap();
    pipeline.add(&dscverifier).unwrap();
    pipeline.add(&decoder).unwrap();
    dsc_caps.sync_state_with_parent().unwrap();
    dscverifier.sync_state_with_parent().unwrap();
    decoder.sync_state_with_parent().unwrap();

    gst::Element::link_many([&h265parse, &dsc_caps, &dscverifier, &decoder]).unwrap();

    println!("  -> linked DSC verifier chain for pad {}", pad_name);

    decoder
}

fn link_audio(q: &gst::Element, pipeline: &gst::Pipeline, pad_name: &str) {
    let ac = gst::ElementFactory::make("audioconvert")
        .name(format!("audioconvert_{}", pad_name))
        .build()
        .unwrap();
    let ar = gst::ElementFactory::make("audioresample")
        .name(format!("audioresample_{}", pad_name))
        .build()
        .unwrap();

    let asink = gst::ElementFactory::make("autoaudiosink")
        .name(format!("audiosink_{}", pad_name))
        .build()
        .or_else(|_| {
            gst::ElementFactory::make("alsasink")
                .name(format!("audiosink_{}", pad_name))
                .build()
        })
        .or_else(|_| {
            gst::ElementFactory::make("pulsesink")
                .name(format!("audiosink_{}", pad_name))
                .build()
        })
        .unwrap_or_else(|_| {
            eprintln!("  -> No audio sink available, using fakesink (silent)");
            gst::ElementFactory::make("fakesink")
                .name(format!("audiosink_{}", pad_name))
                .build()
                .unwrap()
        });

    pipeline.add(&ac).unwrap();
    pipeline.add(&ar).unwrap();
    pipeline.add(&asink).unwrap();
    ac.sync_state_with_parent().unwrap();
    ar.sync_state_with_parent().unwrap();
    asink.sync_state_with_parent().unwrap();

    let acsink = ac.static_pad("sink").unwrap();
    let qsrc = q.static_pad("src").unwrap();
    qsrc.link(&acsink).unwrap();
    gst::Element::link_many([&ac, &ar, &asink]).unwrap();
}

pub fn run_gtk(app: &gtk::Application, whep_endpoint: &str, dsc: &DscConfig) -> Result<()> {
    use gtk::prelude::*;

    let window = gtk::ApplicationWindow::new(app);
    window.set_title(Some("C2PA-DSC — WebRTC Broadcasting Showcase"));
    window.set_default_size(1400, 900);

    let player_ui = PlayerUi::new(1400, 900, whep_endpoint);
    let ui_container = player_ui.container.clone();
    window.set_child(Some(&ui_container));
    window.present();

    let (status_tx, status_rx) = std::sync::mpsc::channel::<DscVerificationResult>();
    let ui_rc = std::rc::Rc::new(std::cell::RefCell::new(player_ui));
    let ur = ui_rc.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
        let u = ur.borrow();
        while let Ok(r) = status_rx.try_recv() {
            u.update(&r);
        }
        glib::ControlFlow::Continue
    });

    let painter = gst::ElementFactory::make("gtk4paintablesink")
        .name("gtksink")
        .build();

    if let Ok(ref painter) = painter {
        let paintable = painter.property::<Option<gdk::Paintable>>("paintable");
        if let Some(paintable) = paintable {
            let picture = gtk::Picture::builder()
                .paintable(&paintable)
                .can_shrink(true)
                .content_fit(gtk::ContentFit::Contain)
                .hexpand(false)
                .vexpand(true)
                .build();
            let ui = ui_rc.borrow();
            ui.set_video_widget(&picture);
        }
    } else {
        eprintln!("  gtk4paintablesink not available, video will open in separate window");
    }

    let videosink = match painter {
        Ok(p) => p,
        Err(_) => gst::ElementFactory::make("ximagesink")
            .name("display")
            .build()
            .unwrap_or_else(|_| {
                gst::ElementFactory::make("fakesink").name("display").build().unwrap()
            }),
    };

    let pipeline = gst::Pipeline::builder().name("player-gtk").build();
    let whepclientsrc = make_whep_src(whep_endpoint)?;
    let videoconvert = make_element("videoconvert", "convert")?;
    // Cap the displayed resolution at 720p (downscale only) so a large source
    // frame can't force the video widget (and window) to grow.
    let videoscale = make_element("videoscale", "scale")?;
    let scale_caps = gst::ElementFactory::make("capsfilter")
        .name("scale-caps")
        .property(
            "caps",
            gst::Caps::builder("video/x-raw")
                .field("width", gst::IntRange::<i32>::new(1, 1280))
                .field("height", gst::IntRange::<i32>::new(1, 720))
                .build(),
        )
        .build()?;

    pipeline.add_many([&whepclientsrc, &videoconvert, &videoscale, &scale_caps, &videosink])?;
    gst::Element::link_many([&videoconvert, &videoscale, &scale_caps, &videosink])?;
    link_whep_pads(&whepclientsrc, &videoconvert, &pipeline, dsc);

    println!("WHEP player started");
    let pipeline = Arc::new(Mutex::new(Some(pipeline)));
    let pclone = pipeline.clone();
    window.connect_destroy(move |_| {
        if let Some(p) = pclone.lock().unwrap().take() {
            let _ = p.set_state(gst::State::Null);
        }
    });

    pipeline.lock().unwrap().as_ref().unwrap().set_state(gst::State::Playing)?;

    let pclone = pipeline.clone();
    std::thread::spawn(move || loop {
        let msg = {
            let guard = pclone.lock().unwrap();
            guard.as_ref().and_then(|p| p.bus())
                .and_then(|b| b.timed_pop(gst::ClockTime::from_mseconds(500)))
        };
        match msg {
            Some(msg) => match msg.view() {
                gst::MessageView::Eos(..) => break,
                gst::MessageView::Error(err) => {
                    eprintln!(
                        "Player GTK error: {} ({})",
                        err.error(),
                        err.debug().unwrap_or_default()
                    );
                    break;
                }
                gst::MessageView::Element(element_msg) => {
                    if let Some(s) = element_msg.structure() {
                        if let Some(r) = extract_dsc_result(s) {
                            let _ = status_tx.send(r);
                        }
                    }
                }
                _ => {}
            },
            None => std::thread::sleep(std::time::Duration::from_millis(100)),
        }
    });

    Ok(())
}
