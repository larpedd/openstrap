use std::{
    env::{self, current_exe},
    fs::{self, File},
    io::{self, Cursor},
    path::PathBuf,
};

use crate::{
    config::{LOCALAPPDATA_NAME, NAME, POST_INSTALL_URL, SETUP, URI, URL, YEARS},
    utils,
};
use anyhow::Result;
use anyhow::anyhow;
use reqwest::Client;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use zip::ZipArchive;

pub async fn is_up_to_update() -> Result<(bool, String)> {
    let client = Client::new();
    let latest_version = client
        .get(format!("{SETUP}/version"))
        .send()
        .await?
        .text()
        .await?;
    let local_appdata = env::var("LOCALAPPDATA")?;
    let install_dir = PathBuf::from(local_appdata).join(LOCALAPPDATA_NAME);

    if fs::read_to_string(install_dir.join("version"))
        .map(|x| x == latest_version)
        .unwrap_or(false)
    {
        Ok((true, latest_version))
    } else {
        Ok((false, latest_version))
    }
}

#[allow(clippy::too_many_lines, reason = "code is more readable as it is")]
pub async fn bootstrap() -> Result<()> {
    let client = Client::new();

    let local_appdata = env::var("LOCALAPPDATA")?;
    let install_dir = PathBuf::from(local_appdata).join(LOCALAPPDATA_NAME);
    let mut is_an_update: bool = false;

    if install_dir.is_dir(){
        paris::log!("{NAME} already installed, Checking for updates...");
        let (up_to_date, latest_version) = is_up_to_update().await?;
        if up_to_date {
            paris::info!("Latest version of {NAME}, {latest_version} installed. Nothing to do.");
            open::that(format!("https://www.{URL}/games"))?;
            return Ok(());
        }
    }

    if install_dir.is_dir(){
        is_an_update = true;
    }

    let latest_version = client
        .get(format!("{SETUP}/version"))
        .send()
        .await?
        .text()
        .await?;

    fs::create_dir_all(&install_dir)?;
    env::set_current_dir(&install_dir)?;

    if is_an_update{
        paris::info!("Updating {NAME} Clients...");
    }
    for year in &YEARS {
        paris::log!("Downloading {year} client...");
        // PLEASE EDIT THIS before PORTing IT TO YOUR REVIVALS
        let url = format!("{SETUP}/{latest_version}-ProjectXApp{year}.zip");
        let res = client.get(&url).send().await?;

        let total_size = res
            .content_length()
            .ok_or_else(|| anyhow!("Failed to get content length"))?;

        let pb = ProgressBar::new(total_size);
        pb.set_style(ProgressStyle::default_bar()
            .template("{msg}\n{percent_precise}% |{bar}| {bytes}/{total_bytes} [{elapsed_precise}<{eta_precise}, {decimal_bytes_per_sec}]\n")?
            .progress_chars("█▌ "));
        pb.set_message(format!("Downloading..."));

        let mut downloaded: u64 = 0;
        let mut stream = res.bytes_stream();
        let mut body = Vec::with_capacity(total_size as usize);

        while let Some(item) = stream.next().await {
            let chunk = item?;
            body.extend_from_slice(&chunk);
            
            downloaded += chunk.len() as u64;
            pb.set_position(downloaded);
        }

        pb.finish();
        paris::success!("Downloaded {year} client.");

        let bytes = body;

        paris::info!("Extracting client...");
        let client_path = PathBuf::from(format!("Versions/{latest_version}/{year}"));
        fs::create_dir_all(&client_path)?;
        let reader = Cursor::new(bytes);

        let mut zip = ZipArchive::new(reader)?;
        let pb = ProgressBar::new(zip.len() as u64);
        pb.set_style(ProgressStyle::default_bar()
            .template("{msg}\n{percent}% |{bar}| {pos}/{len} [{elapsed_precise}<{eta_precise}, {per_sec}]\n")?
            .progress_chars("█▌ "));
        pb.set_message("Extracting...");

        for i in 0..zip.len() {
            let Ok(mut file) = zip.by_index(i) else {
                paris::warn!("Failed to extract file at index {i}, this client may not work!");
                continue;
            };
            let Some(path) = file.enclosed_name() else {
                paris::warn!(
                    "Filed at index {i} has invalid path and can't be extracted, this client may not work!"
                );
                continue;
            };
            let path = client_path.join(path);
            if file.is_dir() {
                match fs::create_dir_all(&path) {
                    Ok(()) => {}
                    Err(e) => {
                        paris::error!(
                            "Failed to create directory {path:?} ({e}), this client may not work!"
                        );
                    }
                }
            } else {
                if let Some(path) = path.parent() && !path.exists() {
                    match fs::create_dir_all(path) {
                        Ok(()) => {}
                        Err(e) => {
                            paris::error!(
                                "Failed to create directory {path:?} ({e}), this client may not work!"
                            );
                        }
                    }
                }
                let Ok(mut fsfile) = File::create(&path) else {
                    paris::error!("Failed to create file {path:?}");
                    continue;
                };
                match io::copy(&mut file, &mut fsfile) {
                    Ok(_) => {}
                    Err(e) => {
                        paris::error!("Failed to create file {path:?}\n{e}");
                    }
                }
            }
            pb.inc(1);
        }
        pb.finish_with_message("Extracted client.");
        paris::success!("Successfully installed the {year} client");
    }
    fs::write("version", latest_version)?;

    if is_an_update {
        paris::success!("All {} clients has been updated.", NAME);
    }else{
        paris::success!("All {} clients installed", NAME);
    }
    paris::log!("Copying self to folder");
    let launcher_path = install_dir.join("Launcher.exe");
    fs::copy(current_exe()?, &launcher_path)?;

    paris::log!("Setting up registry");
    utils::register_uri(URI, &launcher_path)
        .map_err(|e| anyhow!("Error registering URI: {e}"))?;
    utils::add_uninstall_shortcut(&launcher_path)
        .map_err(|e| anyhow!("Error adding uninstall shortcut: {e}"))?;
    paris::success!("Finished installing {NAME}, have fun playing! ^_^");

    if !is_an_update {
        open::that(POST_INSTALL_URL)?;
    }
    Ok(())
}
