#![warn(clippy::pedantic)]
use std::{
    env,
    io::{Write},
    thread,
    time::Duration,
};

use figlet_rs::FIGfont;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

mod bootstrapper;
mod config;
mod launcher;
mod utils;
mod uninstall;

#[tokio::main]
async fn main() {
    let font = FIGfont::from_content(config::FIGLET_FONT).unwrap();
    let figlet_text = font.convert(config::NAME).unwrap().to_string();

    let mut stdout = StandardStream::stdout(ColorChoice::Auto);
    stdout
        .set_color(ColorSpec::new().set_fg(Some(Color::White)))
        .unwrap();
    write!(&mut stdout, "{figlet_text}").unwrap();
    stdout.reset().unwrap();
    println!();
    stdout
        .set_color(ColorSpec::new().set_fg(Some(Color::Yellow)))
        .unwrap();
    write!(&mut stdout, "URL: https://www.{}/", config::URL).unwrap();
    stdout.reset().unwrap();
    println!();

    let mut args = env::args();
    let _ = args.next();
    match args.next() {
        None => {
            if let Err(err) = bootstrapper::bootstrap().await {
                paris::error!("Error while bootstrapping: {err:?}");
            }
        }
        Some(x) if x.starts_with(config::URI) => {
            if let Err(err) = launcher::launch(&x).await {
                paris::error!("Error while launching: {err:?}");
                paris::log!("Closing in 5 seconds");
            } else {
                paris::log!("Closing in 5 seconds");
            }
            thread::sleep(Duration::from_secs(5));
        }
        Some(x) if x.starts_with("uninstall") => {
            if let Err(err) = uninstall::main().await {
                paris::error!("Error while uninstalling: {err:?}");
            }
        }
        Some(x) => {
            paris::error!("Unknown argument: {x}");
        }
    }
}
