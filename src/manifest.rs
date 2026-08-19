use anyhow::{Context, Result};
use std::process::Command;
use std::sync::{Arc, Mutex};
use warp::Filter;

use std::path::PathBuf;

const FAKE_UUID: &str = "ffffffffffffffffffffffffffffffff";

pub fn generate_fake_manifest(title: &str) -> Result<Vec<u8>> {
    // Return cached version if it exists
    let cache_path = format!("/tmp/fake-manifest-{:x}.c2pa", 
        title.as_bytes().iter().map(|b| *b as u32).sum::<u32>());
    if let Ok(data) = std::fs::read(&cache_path) {
        if !data.is_empty() {
            return Ok(data);
        }
    }

    println!("Generating fake manifest (uuid={}, title={})...", FAKE_UUID, title);

    let script = format!(
        "#!/bin/bash\ngst-launch-1.0 -e \\\n\
         videotestsrc num-buffers=60 ! \\\n\
         video/x-raw,width=176,height=144 ! \\\n\
         videoconvert ! \\\n\
         vah265enc key-int-max=30 ! \\\n\
         h265parse ! \\\n\
         video/x-h265,stream-format=hvc1,alignment=au ! \\\n\
         dscsigner enable-c2pa=true \\\n\
             c2pa-manifest-json='{{\\\"title\\\":\\\"{}\\\"}}' \\\n\
             private-key-path=/tmp/c2pa-certs/provider.key \\\n\
             public-key-uri=file:///tmp/c2pa-certs/provider.crt \\\n\
             content-uuid={} \\\n\
             substream-length=30 hash-method=sha256 ! \\\n\
         fakesink\n",
        title, FAKE_UUID
    );

    let script_path = "/tmp/gen-fake-manifest.sh";
    std::fs::write(script_path, &script)?;

    let output = Command::new("bash")
        .arg(script_path)
        .output()
        .context("Failed to run fake manifest generation script")?;

    let _ = std::fs::remove_file(script_path);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Fake manifest script failed: {}", stderr);
    }

    let path = format!("/tmp/dsc-c2pa-{}.c2pa", FAKE_UUID);
    let data = std::fs::read(&path)
        .with_context(|| format!("Fake manifest not found at {}", path))?;

    std::fs::write(&cache_path, &data).ok();
    let _ = std::fs::remove_file(&path);
    println!("Fake manifest generated ({} bytes)", data.len());
    Ok(data)
}

pub fn start_server(
    port: u16,
    fake_manifest: Arc<Mutex<Vec<u8>>>,
    certs_dir: PathBuf,
) -> Arc<Mutex<bool>> {
    let serving_original = Arc::new(Mutex::new(true));

    let state = serving_original.clone();
    let fake = fake_manifest;

    let certs_route = warp::path("certs")
        .and(warp::fs::dir(certs_dir.clone()));

    let route = certs_route.or(warp::path::tail()
        .and(warp::any().map(move || state.clone()))
        .and_then(move |tail: warp::path::Tail, state: Arc<Mutex<bool>>| {
            let fake = fake.clone();
            async move {
                let orig = state.lock().unwrap();
                if !*orig {
                    let data = fake.lock().unwrap().clone();
                    if data.is_empty() {
                        return Ok::<_, warp::Rejection>(warp::http::Response::builder()
                            .status(404)
                            .body("manifest not available".into()));
                    }
                    return Ok(warp::http::Response::builder()
                        .header("Content-Type", "application/octet-stream")
                        .header("Access-Control-Allow-Origin", "*")
                        .body(data));
                }
                // Serve the exact manifest requested by filename
                // (e.g. /dsc-c2pa-{uuid}.c2pa), so a stale manifest from a
                // previous session can never be served for a new stream.
                let filename = tail.as_str().trim_start_matches('/');
                if filename.starts_with("dsc-c2pa-") && !filename.contains("..") {
                    let path = std::path::Path::new("/tmp").join(filename);
                    if let Ok(data) = std::fs::read(&path) {
                        return Ok(warp::http::Response::builder()
                            .header("Content-Type", "application/c2pa")
                            .header("Access-Control-Allow-Origin", "*")
                            .body(data));
                    }
                }
                // Fallback: serve fake manifest until real one is available
                {
                    let data = fake.lock().unwrap().clone();
                    if !data.is_empty() {
                        return Ok(warp::http::Response::builder()
                            .header("Content-Type", "application/octet-stream")
                            .header("Access-Control-Allow-Origin", "*")
                            .body(data));
                    }
                }
                Ok::<_, warp::Rejection>(warp::http::Response::builder()
                    .status(404)
                    .body("manifest not found yet".into()))
            }
        }));

    println!("Serving certs from {} at /certs/", certs_dir.display());

    let addr: std::net::SocketAddr = ([0, 0, 0, 0], port).into();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            warp::serve(route).try_bind(addr).await;
        });
    });

    serving_original
}
