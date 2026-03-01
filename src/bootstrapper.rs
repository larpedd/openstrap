use std::{
    env::{self, current_exe},
    fs::{self, File},
    io::{self, Cursor},
    path::PathBuf,
    process::Command,
    time::Duration,
};

use crate::{config::*, utils};
use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use zip::ZipArchive;

const MAX_RETRIES: u32 = 3;
const RETRY_DELAY: Duration = Duration::from_secs(2);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

fn build_client() -> Result<Client> {
    Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .tcp_keepalive(Duration::from_secs(10))
        .build()
        .context("Failed to build HTTP client")
}

pub fn get_install_dir() -> Result<PathBuf> {
    #[cfg(windows)]
    {
        let local_appdata = env::var("LOCALAPPDATA").context("LOCALAPPDATA not set")?;
        Ok(PathBuf::from(local_appdata).join(LOCALAPPDATA_NAME))
    }
    #[cfg(target_os = "linux")]
    {
        let home = env::var("HOME").context("HOME not set")?;
        Ok(PathBuf::from(home).join(format!(".local/share/{LOCALAPPDATA_NAME}")))
    }
}

async fn fetch_latest_version(client: &Client) -> Result<String> {
    let version = client
        .get(format!("{SETUP}/version"))
        .send()
        .await
        .context("Failed to reach version endpoint")?
        .error_for_status()
        .context("Version endpoint returned error")?
        .text()
        .await
        .context("Failed to read version response")?
        .trim()
        .to_string();

    if version.is_empty() {
        return Err(anyhow!("Server returned an empty version string"));
    }

    Ok(version)
}

pub async fn is_up_to_update() -> Result<(bool, String)> {
    let client = Client::new();
    let install_dir = get_install_dir()?;
    let latest_version = fetch_latest_version(&client).await?;

    let up_to_date = fs::read_to_string(install_dir.join("version"))
        .map(|v| v.trim() == latest_version)
        .unwrap_or(false);

    Ok((up_to_date, latest_version))
}

/// Downloads a URL into a `Vec<u8>` with retries and a progress bar.
/// Falls back gracefully if content-length is not provided.
/// refactor from claude (im too dumb)
async fn download_with_retry(client: &Client, url: &str, label: &str) -> Result<Vec<u8>> {
    let mut last_err = anyhow!("No attempts made");

    for attempt in 1..=MAX_RETRIES {
        if attempt > 1 {
            paris::warn!("Retry {}/{MAX_RETRIES} for {label}...", attempt);
            tokio::time::sleep(RETRY_DELAY).await;
        }

        match try_download(client, url, label).await {
            Ok(bytes) => return Ok(bytes),
            Err(e) => {
                paris::error!("Download attempt {attempt} failed: {e}");
                last_err = e;
            }
        }
    }

    Err(last_err).with_context(|| format!("All {MAX_RETRIES} download attempts failed for {label}"))
}

async fn try_download(client: &Client, url: &str, label: &str) -> Result<Vec<u8>> {
    let res = client
        .get(url)
        .send()
        .await
        .context("Failed to send request")?
        .error_for_status()
        .context("Server returned error status")?;

    let total_size = res.content_length(); // Optional — not required

    let pb = ProgressBar::new(total_size.unwrap_or(0));
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{msg}\n{percent_precise}% |{bar}| {bytes}/{total_bytes} [{elapsed_precise}<{eta_precise}, {decimal_bytes_per_sec}]\n")?
            .progress_chars("█▌ "),
    );
    pb.set_message(format!("Downloading {label}..."));

    let mut body = match total_size {
        Some(size) => Vec::with_capacity(size as usize),
        None => Vec::new(),
    };

    let mut stream = res.bytes_stream();
    while let Some(item) = stream.next().await {
        let chunk = item.context("Stream error while downloading")?;
        body.extend_from_slice(&chunk);
        pb.inc(chunk.len() as u64);
    }

    // Validate we got what we expected
    if let Some(expected) = total_size {
        if body.len() as u64 != expected {
            return Err(anyhow!(
                "Download incomplete: got {} bytes, expected {}",
                body.len(),
                expected
            ));
        }
    }

    pb.finish_and_clear();
    Ok(body)
}


fn extract_zip(bytes: Vec<u8>, dest: &PathBuf, label: &str) -> Result<()> {
    paris::info!("Extracting {label} client...");
    fs::create_dir_all(dest)
        .with_context(|| format!("Failed to create directory {dest:?}"))?;

    let reader = Cursor::new(bytes);
    let mut zip = ZipArchive::new(reader).context("Failed to open zip archive")?;

    let pb = ProgressBar::new(zip.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{msg}\n{percent}% |{bar}| {pos}/{len} [{elapsed_precise}<{eta_precise}, {per_sec}]\n")?
            .progress_chars("█▌ "),
    );
    pb.set_message("Extracting...");

    for i in 0..zip.len() {
        let Ok(mut file) = zip.by_index(i) else {
            paris::warn!("Failed to extract file at index {i}, skipping");
            pb.inc(1);
            continue;
        };

        let Some(rel_path) = file.enclosed_name() else {
            paris::warn!("File at index {i} has an unsafe path, skipping");
            pb.inc(1);
            continue;
        };

        let path = dest.join(rel_path);

        if file.is_dir() {
            if let Err(e) = fs::create_dir_all(&path) {
                paris::error!("Failed to create dir {path:?}: {e}");
            }
        } else {
            if let Some(parent) = path.parent() {
                if !parent.exists() {
                    if let Err(e) = fs::create_dir_all(parent) {
                        paris::error!("Failed to create parent dir {parent:?}: {e}");
                        pb.inc(1);
                        continue;
                    }
                }
            }

            match File::create(&path) {
                Ok(mut fsfile) => {
                    if let Err(e) = io::copy(&mut file, &mut fsfile) {
                        paris::error!("Failed to write {path:?}: {e}");
                    }
                }
                Err(e) => paris::error!("Failed to create {path:?}: {e}"),
            }
        }

        pb.inc(1);
    }

    pb.finish_with_message(format!("Extracted {label} client."));
    Ok(())
}

#[cfg(target_os = "linux")]
fn check_wine() -> Result<()> {
    use std::path::Path;

    paris::log!("Checking for wine...");
    let output = Command::new("wine")
        .arg("--version")
        .output()
        .context("Failed to execute wine, is it installed?")?;

    if output.status.success() {
        paris::info!(
            "Wine detected: {}",
            String::from_utf8_lossy(&output.stdout).trim()
        );
    } else {
        return Err(anyhow!(
            "Wine check failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let home = env::var("HOME").context("HOME not set")?;
    let dot_wine = Path::new(&home).join(".wine");

    if dot_wine.is_dir() {
        paris::info!("Detected .wine folder.");
    } else {
        paris::info!(".wine folder not found. Running wineboot...");
        let output = Command::new("wineboot")
            .output()
            .context("Failed to run wineboot")?;
        if output.status.success() {
            paris::info!("Wine prefix initialized.");
        } else {
            return Err(anyhow!(
                "wineboot failed:\n{}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }

    Ok(())
}

#[allow(clippy::too_many_lines, reason = "code is more readable as it is")]
pub async fn bootstrap() -> Result<()> {
    let client = build_client()?;
    let install_dir = get_install_dir()?;

    let is_an_update: bool = if install_dir.is_dir(){
        paris::log!("{NAME} already installed, Checking for updates...");
        let (up_to_date, latest_version) = is_up_to_update().await?;
        if up_to_date {
            paris::info!("Latest version of {NAME}, {latest_version} installed. Nothing to do.");
            open::that(format!("https://www.{URL}/games"))?;
            return Ok(());
        }
        true
    } else {
        false
    };

    #[cfg(target_os = "linux")]
    check_wine()?;

    let latest_version = fetch_latest_version(&client).await?;

    fs::create_dir_all(&install_dir)
        .with_context(|| format!("Failed to create install dir {install_dir:?}"))?;
    env::set_current_dir(&install_dir)?;

    if is_an_update {
        paris::info!("Updating {NAME} clients to {latest_version}...");
    } else {
        paris::info!("Installing {NAME} {latest_version}...");
    }

    for year in &YEARS {
        let url = format!("{SETUP}/{latest_version}-{CLIENTFILENAMEPREFIX}{year}.zip");

        paris::log!("Downloading {year} client...");
        let bytes = download_with_retry(&client, &url, &year.to_string()).await
            .with_context(|| format!("Failed to download {year} client"))?;
        paris::success!("Downloaded {year} client.");

        let client_path = PathBuf::from(format!("Versions/{latest_version}/{year}"));
        extract_zip(bytes, &client_path, &year.to_string())
            .with_context(|| format!("Failed to extract {year} client"))?;
        paris::success!("Installed {year} client.");
    }
    fs::write("version", &latest_version).context("Failed to write version file")?;

    paris::log!("Copying self to install directory...");
    #[cfg(windows)]
    let launcher_path = install_dir.join("launcher.exe");
    #[cfg(target_os = "linux")]
    let launcher_path = install_dir.join("launcher");
    fs::copy(current_exe()?, &launcher_path).context("Failed to copy launcher")?;

    
    paris::log!("Setting up Launcher and uninstall shortcut...");
    utils::register_uri(URI, &launcher_path)
        .map_err(|e| anyhow!("Failed to register URI handler: {e}"))?;
    utils::add_uninstall_shortcut(&launcher_path)
        .map_err(|e| anyhow!("Failed to add uninstall shortcut: {e}"))?;

    if is_an_update {
        paris::success!("All {NAME} clients updated to {latest_version}.");
    } else {
        paris::success!("All {NAME} clients installed. Have fun! :3");
        open::that(POST_INSTALL_URL)?;
    }
    Ok(())
}
