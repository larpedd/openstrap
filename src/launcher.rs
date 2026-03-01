use std::{
    process::Command
};

use anyhow::Result;

use crate::bootstrapper;
use crate::{
    config::{URI, URL}
};

struct Args {
    _launch_mode: String,
    client_version: String,
    game_info: String,
    place_launcher_url: String,
}

pub async fn launch(uri: &str) -> Result<()> {
    let (up_to_date, latest_version) = bootstrapper::is_up_to_update().await?;
    if !up_to_date {
        paris::info!("Out ouf date, updating...");
        bootstrapper::bootstrap().await?;
    }
    if !uri.starts_with(&format!("{URI}:")) {
        anyhow::bail!("Invalid URI");
    }
    let re = regex::Regex::new(&format!(r"{URI}:1\+launchmode:([^+]+)\+clientversion:([^+]+)\+gameinfo:([^+]+)\+placelauncherurl:([^+]+)")).unwrap();
    let captures = re
        .captures(uri)
        .ok_or_else(|| anyhow::anyhow!("Invalid URI format"))?;

    let args: Args = Args {
        _launch_mode: captures.get(1).unwrap().as_str().to_string(),
        client_version: captures.get(2).unwrap().as_str().to_string(),
        game_info: captures.get(3).unwrap().as_str().to_string(),
        place_launcher_url: captures.get(4).unwrap().as_str().to_string(),
    };
    paris::info!("Starting {}", args.client_version);
    let install_path = bootstrapper::get_install_dir()?;
    let client_path = install_path.join("Versions").join(&latest_version).join(&args.client_version).join("ProjectXPlayerBeta.exe");
    #[cfg(windows)]
    Command::new(client_path)
        .arg("--play")
        .arg("-a")
        .arg(&format!("https://www.{URL}/Login/Negotiate.ashx"))
        .arg("-t")
        .arg(args.game_info)
        .arg("-j")
        .arg(args.place_launcher_url)
        .spawn()?;
    #[cfg(target_os = "linux")]
    Command::new("wine")
        .arg(client_path)
        .arg("--play")
        .arg("-a")
        .arg(&format!("https://www.{URL}/Login/Negotiate.ashx"))
        .arg("-t")
        .arg(args.game_info)
        .arg("-j")
        .arg(args.place_launcher_url)
        .spawn()?;
    
    paris::success!("Started Client");
    Ok(())
}
