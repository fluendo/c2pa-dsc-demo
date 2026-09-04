use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone)]
pub struct DscConfig {
    pub private_key_path: PathBuf,
    pub cert_path: PathBuf,
    pub trust_store_path: PathBuf,
    pub key_store_dir: PathBuf,
    pub substream_length: u32,
    pub hash_method: String,
    pub content_uuid: Option<String>,
    pub camera_device: Option<String>,
    pub manifest_uri_template: Option<String>,
    pub public_key_uri: Option<String>,
    pub demo_ai_filter: bool,
    pub ai_effect: u32,
    pub ai_effect_intensity: f32,
    pub ai_model_path: String,
    pub software_encoder: bool,
}

pub struct CertPaths {
    pub ca_cert: PathBuf,
    pub private_key: PathBuf,
    pub cert: PathBuf,
    pub trust_store: PathBuf,
}

impl CertPaths {
    pub fn new(certs_dir: &Path) -> Self {
        Self {
            ca_cert: certs_dir.join("ca.crt"),
            private_key: certs_dir.join("provider.key"),
            cert: certs_dir.join("provider.crt"),
            trust_store: certs_dir.join("ca.crt"),
        }
    }

    pub fn impersonator(certs_dir: &Path) -> Self {
        Self {
            ca_cert: certs_dir.join("ca.crt"),
            private_key: certs_dir.join("impersonator.key"),
            cert: certs_dir.join("impersonator.crt"),
            trust_store: certs_dir.join("ca.crt"),
        }
    }
}

fn run_openssl(args: &[&str]) -> Result<()> {
    let output = Command::new("openssl")
        .args(args)
        .output()
        .context("failed to run openssl")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("openssl failed: {}", stderr);
    }
    Ok(())
}

pub fn ensure_certs(certs: &CertPaths, openssl_config: &Path) -> Result<()> {
    if certs.ca_cert.exists()
        && certs.private_key.exists()
        && certs.cert.exists()
    {
        // The files exist, but they may be owned by root (created by a Docker
        // run that mounted /tmp). Verify the private key is actually readable,
        // otherwise the DSC signer fails with a cryptic "Failed to activate pad".
        if std::fs::read(&certs.private_key).is_err() {
            let dir = certs.private_key.parent().unwrap_or(Path::new("/tmp"));
            anyhow::bail!(
                "Certificate files exist at {} but are not readable \
                 (probably created by Docker as root). Remove them and re-run:\n\
                 \n  sudo rm -rf {}\n",
                certs.private_key.display(),
                dir.display()
            );
        }
        return Ok(());
    }

    let dir = certs.ca_cert.parent().unwrap();
    std::fs::create_dir_all(dir)?;

    println!("Generating C2PA-compatible certificates...");

    if !openssl_config.exists() {
        anyhow::bail!(
            "OpenSSL config not found at {}. \
             Required for C2PA-compatible cert generation.",
            openssl_config.display()
        );
    }

    let dir_str = dir.to_string_lossy();

    run_openssl(&[
        "req", "-x509", "-newkey", "rsa:2048", "-nodes",
        "-keyout", &format!("{}/ca.key", dir_str),
        "-out", &format!("{}/ca.crt", dir_str),
        "-days", "3650",
        "-extensions", "v3_ca",
        "-config", &openssl_config.to_string_lossy(),
        "-subj", "/C=ES/O=Fluendo S.A./CN=Fluendo DSC Root CA",
    ])?;
    println!("  -> Generated CA cert: {}/ca.crt", dir_str);

    let csr_path = format!("{}/provider.csr", dir_str);
    run_openssl(&[
        "req", "-new", "-newkey", "rsa:2048", "-nodes",
        "-keyout", &format!("{}/provider.key", dir_str),
        "-out", &csr_path,
        "-extensions", "v3_req",
        "-config", &openssl_config.to_string_lossy(),
        "-subj", "/C=ES/O=Fluendo S.A./CN=Fluendo DSC Signer",
    ])?;
    println!("  -> Generated provider key: {}/provider.key", dir_str);

    run_openssl(&[
        "x509", "-req",
        "-in", &csr_path,
        "-CA", &format!("{}/ca.crt", dir_str),
        "-CAkey", &format!("{}/ca.key", dir_str),
        "-CAcreateserial",
        "-out", &format!("{}/provider.crt", dir_str),
        "-days", "365",
        "-extensions", "v3_end_entity",
        "-extfile", &openssl_config.to_string_lossy(),
    ])?;

    let _ = std::fs::remove_file(&csr_path);
    println!("  -> Generated provider cert: {}/provider.crt", dir_str);
    println!("Certificates generated successfully.");

    Ok(())
}

pub fn ensure_impersonator_certs(certs: &CertPaths) -> Result<()> {
    if certs.private_key.exists() && certs.cert.exists() {
        return Ok(());
    }

    let dir = certs.private_key.parent().unwrap();
    let dir_str = dir.to_string_lossy();
    println!("Generating impersonator certificate (self-signed, NOT in trust chain)...");

    run_openssl(&[
        "req", "-x509", "-newkey", "rsa:2048", "-nodes",
        "-keyout", &format!("{}/impersonator.key", dir_str),
        "-out", &format!("{}/impersonator.crt", dir_str),
        "-days", "365",
        "-subj", "/C=XX/O=Evil Corp/CN=Fake DSC Signer",
    ])?;
    println!("  -> Generated impersonator cert: {}/impersonator.crt", dir_str);
    Ok(())
}
