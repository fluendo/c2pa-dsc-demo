use anyhow::Result;
use gst::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub fn run(
    whip_addr: String,
    whep_addr: String,
    running: Arc<AtomicBool>,
    payloader_ref: Arc<Mutex<Option<gst::Element>>>,
) -> Result<()> {
    let pipeline = gst::Pipeline::builder().name("server-bridge").build();

    let whipserversrc = gst::ElementFactory::make("whipserversrc")
        .name("whipsrc")
        .build()?;

    let whepserversink = gst::ElementFactory::make("whepserversink")
        .name("whepsink")
        .build()?;

    {
        let whip_signaller = whipserversrc
            .dynamic_cast_ref::<gst::ChildProxy>()
            .ok_or_else(|| anyhow::anyhow!("whipserversrc missing ChildProxy"))?
            .child_by_name("signaller")
            .ok_or_else(|| anyhow::anyhow!("whipserversrc missing 'signaller' child"))?;
        whip_signaller.set_property("host-addr", Some(whip_addr.as_str()));
    }
    {
        let whep_signaller = whepserversink
            .dynamic_cast_ref::<gst::ChildProxy>()
            .ok_or_else(|| anyhow::anyhow!("whepserversink missing ChildProxy"))?
            .child_by_name("signaller")
            .ok_or_else(|| anyhow::anyhow!("whepserversink missing 'signaller' child"))?;
        whep_signaller.set_property("host-addr", Some(whep_addr.as_str()));
    }

    pipeline.add_many([&whipserversrc, &whepserversink])?;

    let pipeline_weak = pipeline.downgrade();
    let wsink_weak = whepserversink.downgrade();
    whipserversrc.connect_pad_added(move |_src, pad| {
        let Some(pipeline) = pipeline_weak.upgrade() else {
            return;
        };
        let Some(wsink) = wsink_weak.upgrade() else {
            return;
        };

        let pad_name = pad.name().to_string();
        println!("  -> whipserversrc pad_added: {}", pad_name);

        let is_video = pad_name.contains("video");
        let is_audio = pad_name.contains("audio");

        if !is_video && !is_audio {
            return;
        }

        let q = gst::ElementFactory::make("queue")
            .name(format!("queue_{}", pad_name))
            .build()
            .unwrap();

        pipeline.add(&q).unwrap();
        q.sync_state_with_parent().unwrap();

        let qsink = q.static_pad("sink").unwrap();
        pad.link(&qsink).unwrap();

        let qsrc = q.static_pad("src").unwrap();
        let template = if is_video {
            "video_%u"
        } else {
            "audio_%u"
        };
        let wsink_pad = wsink.request_pad_simple(template).unwrap();
        qsrc.link(&wsink_pad).unwrap();

        println!(
            "  -> linked {} pad to whepserversink",
            if is_video { "video" } else { "audio" }
        );

        if is_video {
            pipeline.debug_to_dot_file(gst::DebugGraphDetails::all(), "server-playing");
        }
    });

    println!("WHIP server listening on {}", whip_addr);
    println!("WHEP server will start once a WHIP stream arrives");

    // Default: skip config-interval=-1 (bitstream modifier, breaks DSC).
    // The payloader reference is stored for live tamper toggling via toggle_tamper().
    let pr = payloader_ref.clone();
    let _handler = whepserversink.connect("payloader-setup", false, move |args| {
        let payloader = args[3].get::<gst::Element>().unwrap();
        let is_rtph265pay = payloader
            .factory()
            .map(|f| f.name().as_str() == "rtph265pay")
            .unwrap_or(false);
        if is_rtph265pay {
            payloader.set_property_from_str("aggregate-mode", "zero-latency");
            *pr.lock().unwrap() = Some(payloader);
            return Some(true.to_value());
        }
        Some(false.to_value())
    });

    pipeline.set_state(gst::State::Playing)?;

    let bus = pipeline.bus().expect("Pipeline should have a bus");
    while running.load(Ordering::SeqCst) {
        if let Some(msg) = bus.timed_pop(gst::ClockTime::from_mseconds(200)) {
            match msg.view() {
                gst::MessageView::Eos(..) => {
                    println!("Server bridge EOS");
                    break;
                }
                gst::MessageView::Error(err) => {
                    eprintln!(
                        "Server bridge error: {} ({})",
                        err.error(),
                        err.debug().unwrap_or_default()
                    );
                    break;
                }
                _ => (),
            }
        }
    }

    pipeline.set_state(gst::State::Null)?;
    println!("Server bridge stopped");
    Ok(())
}

pub fn toggle_tamper(pay: &Arc<Mutex<Option<gst::Element>>>) -> Option<bool> {
    let guard = pay.lock().unwrap();
    let payloader = guard.as_ref()?;
    let current: i32 = payloader.property("config-interval");
    let new = if current == -1 { 0 } else { -1 };
    payloader.set_property("config-interval", new);
    Some(new == -1)
}
