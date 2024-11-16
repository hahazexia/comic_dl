use reqwest::Client;
use url::Url;
use std::fs;
use std::{fs::File, io::Write};
use std::path::Path;
use clap::{Parser, ValueEnum};
use reqwest::header::{HeaderMap, HeaderName, USER_AGENT, REFERER, ORIGIN, HeaderValue};

use std::sync::Arc;
use tokio::sync::Semaphore;
use std::process;

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
    #[arg(short, long, value_enum, default_value_t = DlType::CURRENT)]
    dl_type: DlType,
}

#[derive(Debug, Clone, ValueEnum)]
enum DlType {
    JUAN,
    HUA,
    FANWAI,
    CURRENT,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let url: String = cli.url;
    let element: String = cli.element;
    let attr: String = cli.attr;
    let file: String = cli.file;
    let dl_type: DlType = cli.dl_type;
    let element_selector =  format!("{element} img");
    println!("url is {url}, element_selector is {element_selector}, attr is {attr}, file is {file}");


    match dl_type {
        DlType::CURRENT => {
            handle_current(url, element_selector, attr, file).await;
        }
        DlType::JUAN => {
            // current only support antbyw.com
            if !url.contains("https://www.antbyw.com/") {
                eprintln!("Error: current only support antbyw.com.");
                process::exit(1);
            } else {
                let client = Client::new();
                let response = client.get(&url).send().await.unwrap();
                let html_content = response.text().await.unwrap();

                let document = scraper::Html::parse_document(&html_content);
                let selector_juan_title = &scraper::Selector::parse("h3.uk-alert-warning").unwrap();
                let juan_nav = document.select(selector_juan_title);
                let mut juan_switcher = None;

                for nav in juan_nav {
                    println!("nav {}", nav.html());
                    if let Some(t) = nav.text().next() {
                        if t.contains("单行本") {
                            let mut current_sibling = nav.next_sibling();
                            while let Some(nav_next) = current_sibling {
                                println!("nav name {:?}", nav_next.value());
                                if let Some(nav_next_el) = nav_next.value().as_element() {
                                    if nav_next_el.has_class("uk-switcher", scraper::CaseSensitivity::CaseSensitive) {
                                        juan_switcher = scraper::ElementRef::wrap(nav_next);
                                        break;
                                    }
                                }
                                current_sibling = nav_next.next_sibling();
                            };
                        } else {
                            eprintln!("Error: juan not exist!");
                            process::exit(1);
                        }
                    }
                }

                if let Some(switcher) = juan_switcher {
                    println!("find switcher! name");
                    println!("{}", switcher.html());
                }
            }
        },
        DlType::HUA => todo!(),
        DlType::FANWAI => todo!(),
    }
}

async fn handle_current(url: String, element_selector: String, attr: String, file: String) {
    let client = Client::new();
    let response = client.get(&url).send().await.unwrap();
    let html_content = response.text().await.unwrap();

    let document = scraper::Html::parse_document(&html_content);
    let image_selector = scraper::Selector::parse(&element_selector).unwrap();
    let images = document.select(&image_selector);
    let mut img_v: Vec<&str> = Vec::new();

    for img in images {
        let image = img.value().attr(&attr);

        // println!("{:?}", image);

        match image {
            Some(i) => {
                img_v.push(i);
            },
            None => println!("img is missing!"),
        }
    }

    println!("img length is {}", img_v.len());

    down_img(img_v, &format!("./{}", &file)).await;
}

async fn down_img(url: Vec<&str>, file_path: &str) {
    let _ = fs::create_dir_all(file_path);
    let client = Client::new();
    let domain = handle_url(url[0]);
    let ext = handle_img_extension(url[0]);

    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36"));
    headers.insert(REFERER, HeaderValue::from_str(&domain).unwrap());
    headers.insert(ORIGIN, HeaderValue::from_str(&domain).unwrap());
    headers.insert(HeaderName::from_static("sec-fetch-mode"), HeaderValue::from_static("no-cors"));

    println!("domain is {domain}, ext is {ext}");

    let semaphore = Arc::new(Semaphore::new(10));
    let mut tasks = vec![];

    for (index, i) in url.iter().enumerate() {
        let client = client.clone();
        let headers = headers.clone();
        let file_path = file_path.to_string();
        let semaphore = semaphore.clone();
        let ext = ext.clone();
        let url = i.to_string();

        let task = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();

            println!("downloading {}", url);
            let response = client.get(url)
                .headers(headers)
                .send()
                .await
                .unwrap()
                .bytes()
                .await
                .unwrap();

            let name = format!("{}/{}.{}", file_path, index, ext);
            let path = Path::new(&name);
            let mut file = File::create(path).unwrap();
            file.write_all(&response).unwrap();
        });

        tasks.push(task);
    }

    for task in tasks {
        let _ = task.await;
    }
}

fn handle_url(url_string: &str) -> String {
    let mut res: String = "".to_string();
    if let Ok(url) = Url::parse(url_string) {
        if let Some(d) = url.domain() {
            println!("Domain: {}", d);
            let d_vec = split_string(d, ".");
            let last_two = &d_vec[d_vec.len() - 2..];
            let mut new_array: Vec<&str> = last_two.iter().map(|s| s.as_str()).collect();
            new_array.insert(0, "www");

            res = format!("https://{}", join_strings(new_array, "."));
        } else {
            println!("Domain not found in the URL");
        }
    } else {
        println!("Invalid URL format");
    }
    res
}

fn split_string(input: &str, delimiter: &str) -> Vec<String> {
    input.split(delimiter).map(|s| s.to_string()).collect()
}

fn join_strings(strings: Vec<&str>, delimiter: &str) -> String {
    strings.join(delimiter)
}

fn handle_img_extension(url_string: &str) -> String {
    let mut res = "";
    if let Some(index) = url_string.rfind('.') {
        res = &url_string[index + 1..];
        println!("File extension: {}", res);
    } else {
        println!("File extension not found in the URL");
    }
    res.to_string()
}