use std::path::Path;
use anyhow::Result;
use crate::config::*;
use std::{env, fs};
use std::path::PathBuf;
use std::process::Command;
use std::os::unix::fs::PermissionsExt;

#[cfg(windows)]
use windows_registry::{Value, CURRENT_USER};

pub fn register_uri(uri_scheme: &str, exe_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]{
        let classes_root = CURRENT_USER.create(r"Software\Classes")?;
        let scheme_key = classes_root.create(uri_scheme)?;
        scheme_key.set_value("", &Value::from(format!("URL:{uri_scheme}").as_str()))?;
        scheme_key.set_value("URL Protocol", &Value::from(""))?;
        let shell_key = scheme_key.create("shell")?;
        let open_key = shell_key.create("open")?;
        let command_key = open_key.create("command")?;

        let command = format!("\"{}\" \"%1\"", exe_path.display());
        command_key.set_value("", &Value::from(command.as_str()))?;
        Ok(())
    }
    #[cfg(target_os = "linux")]{


        let home_dir = env::var("HOME")?;
        let applications = PathBuf::from(home_dir).join(format!(".local/share/applications/"));

    let entry_content = format!("[Desktop Entry]
Name={NAME}
Exec={exe_path:?} %u
Type=Application
Version={DESKTOP_ENTRY_VERSION}
MimeType=x-scheme-handler/{uri_scheme}
");
        fs::write(&applications.join(format!("{NAME}.desktop")),entry_content)?;
        fs::set_permissions(&applications.join(format!("{NAME}.desktop")), fs::Permissions::from_mode(0o755))?;


        let _ = Command::new("update-desktop-database")
            .arg(&applications)
            .output()
            .expect("Failed to update desktop database");

        let _ = Command::new("xdg-settings")
            .arg("set")
            .arg("default-url-scheme-handler")
            .arg(uri_scheme)
            .arg(&applications.join(format!("{NAME}.desktop")))
            .output()
            .expect("Failed to set default url scheme");

        Ok(())
    }
}

pub fn add_uninstall_shortcut(exe_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]{
        let classes_root = CURRENT_USER.create(r"Software\Microsoft\Windows\CurrentVersion\Uninstall")?;
        let app_key = classes_root.create(config::NAME)?;

        let version = env!("CARGO_PKG_VERSION");
        app_key.set_value("DisplayName", &Value::from(config::NAME))?;
        app_key.set_value("Publisher", &Value::from(config::AUTHOR))?;
        app_key.set_value("Version", &Value::from(version))?;
        app_key.set_value("URLInfoAbout", &Value::from(format!("https://www.{}/", config::URL).as_str()))?;
        app_key.set_value("UninstallString", &Value::from(format!("{} uninstall", exe_path.to_string_lossy().into_owned()).as_str()))?;
    }
    #[cfg(target_os = "linux")]{
        let home_dir = env::var("HOME")?;
        let applications = PathBuf::from(home_dir).join(format!(".local/share/applications/"));
        
        let entry_content = format!("[Desktop Entry]
Name=Uninstal {NAME}
Exec={exe_path:?} uninstall
Type=Application
Version={DESKTOP_ENTRY_VERSION}
");
        fs::write(&applications.join(format!("{NAME}-Uninstall.desktop")),entry_content)?;
        fs::set_permissions(&applications.join(format!("{NAME}.desktop")), fs::Permissions::from_mode(0o755))?;

        let _ = Command::new("update-desktop-database")
            .arg(&applications)
            .output()
            .expect("Failed to update desktop database");
    }
    Ok(())
}

pub fn remove_uri(uri_scheme: &str) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]{
        let classes_root = CURRENT_USER.open(r"Software\Classes")?;
        let _ = classes_root.remove_tree(uri_scheme);
    }
    #[cfg(target_os = "linux")]{
        let _ = uri_scheme; // avoid warnings :P
        let home_dir = env::var("HOME")?;
        let applications = PathBuf::from(home_dir).join(format!(".local/share/applications/"));
        let _ = fs::remove_file(applications.join(format!("{NAME}.desktop")));
        let _ = Command::new("update-desktop-database")
            .arg(&applications)
            .output()
            .expect("Failed to update desktop database");
    }
    Ok(())
}

pub fn remove_uninstall_shortcut() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]{
        let classes_root = CURRENT_USER.create(r"Software\Microsoft\Windows\CurrentVersion\Uninstall")?;
        let _ = classes_root.remove_tree(config::NAME)?;
    }#[cfg(target_os = "linux")]{
        let home_dir = env::var("HOME")?;
        let applications = PathBuf::from(home_dir).join(format!(".local/share/applications/"));
        let _ = fs::remove_file(applications.join(format!("{NAME}-Uninstall.desktop")));
        let _ = Command::new("update-desktop-database")
            .arg(&applications)
            .output()
            .expect("Failed to update desktop database");
    }

    Ok(())
}