use std::{
    env,
    io::{self, Write}, 
    fs,
};

#[cfg(windows)]
use std::process::Command;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "linux")]
use self_replace::self_delete;

use anyhow::Result;

use walkdir::WalkDir;
use indicatif::{ProgressBar, ProgressStyle};
use crate::{
    bootstrapper::*, config::*, utils
};


pub async fn main() -> Result<()> {
    let install_dir = get_install_dir()?;
    let current_exe = env::current_exe()?;
    let mut uninstall_from_boostrapper_installer = true; // i.e. running the binary outside the installation folder.

    if install_dir.is_dir(){
        let mut option: String = String::new();
        print!("You are about to uninstall {}.\nAre you sure to continue? (y/N): ", NAME);
        io::stdout().flush()?;
        io::stdin()
            .read_line(&mut option)
            .expect("Failed to read line");

        let option = option.trim().to_lowercase();
        if option == "yes" || option == "y"{
            paris::info!("Starting...");

            let total_files_dir = WalkDir::new(&install_dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .count();

            paris::info!("Removing {} files and directories...", total_files_dir);

            let pb = ProgressBar::new(total_files_dir as u64);
            pb.set_style(ProgressStyle::default_bar()
                .template("{msg}\n{percent}% |{bar}| {human_pos}/{human_len} [{elapsed_precise}<{eta_precise}, {per_sec}]\n")?
                .progress_chars("█▌ "));

            let mut removed = 0;

            for entry in WalkDir::new(&install_dir).contents_first(true) {
                let entry = entry?;
                let path = entry.path();

                if path == current_exe {
                    uninstall_from_boostrapper_installer = false;
                    continue;
                }

                let res = if entry.file_type().is_dir() {
                    fs::remove_dir(path)
                } else {
                    fs::remove_file(path)
                };

                match res {
                    Ok(_) => {
                        removed += 1;
                        pb.set_position(removed);
                        pb.set_message(format!("Removing {:?}...", path));
                    }
                    Err(e) => {
                        if path == install_dir {
                            continue;
                        }
                        paris::error!("Failed to remove {path:?} ({e})");
                    }
                }
            }

            pb.finish();

            paris::success!("Successfully removing {} clients", NAME);

            paris::log!("Removing URI...");

            if let Err(e) = utils::remove_uri(URI) {
                paris::warn!("Failed to remove URI {}", e);
            } else {
                paris::success!("URI removed.");
            }

            paris::log!("Removing uninstall shortcut...");

            if let Err(e) = utils::remove_uninstall_shortcut() {
                paris::warn!("Failed to remove shortcut {}", e);
            } else {
                paris::success!("Shortcut removed.");
            }

            #[cfg(target_os = "linux")]
            if !uninstall_from_boostrapper_installer{
                self_delete()?;
            }

            paris::success!("{} is uninstalled.",NAME);
            print!("Press Enter to continue...");
            io::stdout().flush().unwrap();
            io::stdin().read_line(&mut String::new()).expect(&String::new());

            #[cfg(windows)]
            if !uninstall_from_boostrapper_installer{
                let _ = Command::new("cmd")
                    .raw_arg(format!(" /C ping 127.0.0.1 -n 3 > nul & del \"{}\" & rmdir \"{}\"", current_exe.display(), install_dir.display()))
                    .spawn();
            }
            return Ok(());
        } else {
            paris::info!("Aborted.");
            return Ok(());
        }
    } else {
        paris::success!("{} client already uninstalled, no need to worry.", NAME)
    }
    Ok(())
}