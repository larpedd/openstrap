use std::path::Path;
use windows_registry::*;
use anyhow::Result;

use crate::config;

pub fn register_uri(uri_scheme: &str, exe_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
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

pub fn add_uninstall_shortcut(exe_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let classes_root = CURRENT_USER.create(r"Software\Microsoft\Windows\CurrentVersion\Uninstall")?;
    let app_key = classes_root.create(config::NAME)?;

    let version = env!("CARGO_PKG_VERSION");
    app_key.set_value("DisplayName", &Value::from(config::NAME))?;
    app_key.set_value("Publisher", &Value::from(config::AUTHOR))?;
    app_key.set_value("Version", &Value::from(version))?;
    app_key.set_value("URLInfoAbout", &Value::from(format!("https://www.{}/", config::URL).as_str()))?;
    app_key.set_value("UninstallString", &Value::from(format!("{} uninstall", exe_path.to_string_lossy().into_owned()).as_str()))?;

    Ok(())
}

pub fn remove_uri(uri_scheme: &str) -> Result<(), Box<dyn std::error::Error>> {
    let classes_root = CURRENT_USER.open(r"Software\Classes")?;
    let _ = classes_root.remove_tree(uri_scheme);
    
    Ok(())
}

pub fn remove_uninstall_shortcut() -> Result<(), Box<dyn std::error::Error>> {
    let classes_root = CURRENT_USER.create(r"Software\Microsoft\Windows\CurrentVersion\Uninstall")?;
    let _ = classes_root.remove_tree(config::NAME)?;

    Ok(())
}