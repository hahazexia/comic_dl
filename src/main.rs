use std::process;

use clap::Parser;
use colored::Colorize;

mod mangadex;
mod dl_type;
mod utils;
mod local;
mod antbyw;
use local::{handle_upscale, handle_local};
use dl_type::DlType;
use antbyw::{handle_current, handle_juan_hua_fanwai};
use mangadex::handle_mangadex;
use utils::get_second_level_domain;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// comic website url
    #[arg(short, long)]
    url: String,

    /// which element that contains comic images
    #[arg(short, long, default_value_t = (".uk-zjimg").to_string())]
    element: String,

    /// image element src attr
    #[arg(short, long, default_value_t = ("data-src").to_string())]
    attr: String,

    /// save filepath name
    #[arg(short, long, default_value_t = ("./output").to_string())]
    file: String,

    /// download type, "juan" "hua" "fanwai" "current"
    #[arg(short, long, value_enum, default_value_t = DlType::Current)]
    dl_type: DlType,
}

/// range min value of the chapters
// #[arg(short, long, default_value_t = 0.0)]
// small: f32,

/// range min value of the chapters
// #[arg(short, long, default_value_t = std::f32::INFINITY)]
// big: f32,



// const RED: &str = "\x1b[31m";    // 红色
// const GREEN: &str = "\x1b[32m";  // 绿色
// const RESET: &str = "\x1b[0m";   // 重置颜色
// const YELLOW: &str = "\x1b[33m"; // 黄色

// cargo run -- -u "C:\Users\hahaz\Downloads\王者天下_单行本" -d "upscale"
// cargo run -- -u "https://www.antbyw.com/plugin.php?id=jameson_manhua&c=index&a=bofang&kuid=143450" -d "hua"
// cargo run -- -u "https://www.antbyw.com/plugin.php?id=jameson_manhua&a=read&kuid=152174&zjid=916038"


#[tokio::main]
async fn main() {

    let cli = Cli::parse();

    let url: String = cli.url;
    let element: String = cli.element;
    let attr: String = cli.attr;
    let file: String = cli.file;
    let dl_type: DlType = cli.dl_type;
    // let min: f32 = cli.small;
    // let max: f32 = cli.big;
    let element_selector = format!("{element} img");
    println!(
        "{}is {}, {}is {}, {}is {}, {}is {}",
        "url ".purple(),
        url,
        "element_selector ".purple(),
        element_selector,
        "attr ".purple(),
        attr,
        "file ".purple(),
        file,
    );

    match dl_type {
        DlType::Local => {
            let _ = handle_local(url).await;
            return;
        },
        DlType::Upscale => {
            let _ = handle_upscale(url).await;
            return;
        },
        _ => {}
    }

    let site_name_temp = get_second_level_domain(&url);
    let handled_site_name;

    match site_name_temp {
        Some(site_name) => {
            handled_site_name = site_name;
        }
        None => {
            eprintln!("{}", "get second level domain failed".red());
            process::exit(1);
        }
    }

    match handled_site_name.as_str() {
        "antbyw" => {
            match dl_type {
                DlType::Current => {
                    let _ = handle_current(url, element_selector, attr, file).await;
                }
                DlType::Juan => {
                    handle_juan_hua_fanwai(url, DlType::Juan).await;
                }
                DlType::Hua => {
                    handle_juan_hua_fanwai(url, DlType::Hua).await;
                }
                DlType::Fanwai => {
                    handle_juan_hua_fanwai(url, DlType::Fanwai).await;
                },
                _ => {}
            }
        }
        "mangadex" => {
            let _ = handle_mangadex(url).await;
        }
        _ => {
            eprintln!("{}", "unknown manga site, not support".red());
            process::exit(1);
        }
    }
}
