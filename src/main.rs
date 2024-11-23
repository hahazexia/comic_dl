use clap::{Parser, ValueEnum};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, ORIGIN, REFERER, USER_AGENT};
use reqwest::Client;
use std::fs;
use std::path::Path;
use std::fs::File;
use std::io::Cursor;
use url::Url;
use indicatif::{ProgressBar, ProgressStyle};
use image::ImageFormat;
use std::sync::{Arc, Mutex};
use std::process;
use tokio::sync::Semaphore;
use colored::Colorize;

// cargo run -- -u "" -d ""

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

// const RED: &str = "\x1b[31m";    // 红色
// const GREEN: &str = "\x1b[32m";  // 绿色
// const RESET: &str = "\x1b[0m";   // 重置颜色
// const YELLOW: &str = "\x1b[33m"; // 黄色

// cargo run -- -u "https://www.antbyw.com/plugin.php?id=jameson_manhua&c=index&a=bofang&kuid=152174" -d "juan"
// cargo run -- -u "https://www.antbyw.com/plugin.php?id=jameson_manhua&a=read&kuid=152174&zjid=916039"

#[tokio::main]
async fn main() {

    let cli = Cli::parse();

    let url: String = cli.url;
    let element: String = cli.element;
    let attr: String = cli.attr;
    let file: String = cli.file;
    let dl_type: DlType = cli.dl_type;
    let element_selector = format!("{element} img");
    println!(
        "url is {url}, element_selector is {element_selector}, attr is {attr}, file is {file}"
    );

    match dl_type {
        DlType::CURRENT => {
            handle_current(url, element_selector, attr, file).await;
        }
        DlType::JUAN => {
            handle_juan_hua_fanwai(url, DlType::JUAN).await;
        }
        DlType::HUA => {
            handle_juan_hua_fanwai(url, DlType::HUA).await;
        }
        DlType::FANWAI => {
            handle_juan_hua_fanwai(url, DlType::FANWAI).await;
        },
    }
}

async fn handle_juan_hua_fanwai(url: String, dl_type: DlType) {
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

        let mut comic_name = None;
        let mut comic_name_2 = None;
        let name_selector = &scraper::Selector::parse(".uk-heading-line.mt10.m10.mbn").unwrap();
        let comic_name_temp = document.select(name_selector);
        for name in comic_name_temp {
            comic_name = Some(name.inner_html().replace(" ", "_"));
            comic_name_2 = Some(name.inner_html().replace(" ", "_"));
        }

        let text_to_find = match dl_type {
            DlType::CURRENT => "",
            DlType::JUAN => "单行本",
            DlType::HUA => "单话",
            DlType::FANWAI => "番外篇",
        };

        if let Some(name) = comic_name {
            println!("comic name is {}", name);
            // create juan output directory
            let _ = fs::create_dir_all(&(format!("./{}_{}", &name, text_to_find).replace(" ", "_")));
        } else {
            eprintln!("Error: can not find comic name!");
            process::exit(1);
        }

        for nav in juan_nav {
            if let Some(t) = nav.text().next() {
                if t.contains(text_to_find) {
                    let mut current_sibling = nav.next_sibling();
                    while let Some(nav_next) = current_sibling {
                        if let Some(nav_next_el) = nav_next.value().as_element() {
                            if nav_next_el
                                .has_class("uk-switcher", scraper::CaseSensitivity::CaseSensitive)
                            {
                                juan_switcher = scraper::ElementRef::wrap(nav_next);
                                break;
                            }
                        }
                        current_sibling = nav_next.next_sibling();
                    }
                } else {
                    // println!("this is not juan!");
                    continue;
                }
            }
        }

        if let Some(switcher) = juan_switcher {
            // println!("find switcher! name");
            let mut target: Vec<_> = switcher.select(&scraper::Selector::parse("a.zj-container").unwrap()).collect();

            target.sort_by(|a, b| {
                let a_inner = a.inner_html();
                let b_inner = b.inner_html();

                // 提取数字并进行比较
                let a_number = extract_number(&a_inner);
                let b_number = extract_number(&b_inner);

                a_number.cmp(&b_number)
            });

            for a_btn in target {
                if let Some(src) = a_btn.value().attr("href") {
                    // println!("src is {}, inner is {}", src, a_btn.inner_html());
                    let mut complete_url = String::from(src);
                    complete_url.remove(0);
                    let host = String::from("https://www.antbyw.com");
                    let complete_url = host + &complete_url;
                    println!(
                        "complete_url is {}, name is {}",
                        complete_url,
                        a_btn.inner_html()
                    );

                    if let Some(ref comic_name_temp) = comic_name_2 {
                        handle_current(
                            complete_url,
                            ".uk-zjimg img".to_string(),
                            "data-src".to_string(),
                            format!("./{}_{}/{}", *comic_name_temp, text_to_find, a_btn.inner_html()),
                        )
                        .await;
                    }
                }
            }

            // println!("{}", switcher.html());
            // for a_btn in switcher.select(&scraper::Selector::parse("a.zj-container").unwrap()) {
            //     if let Some(src) = a_btn.value().attr("href") {
            //         // println!("src is {}, inner is {}", src, a_btn.inner_html());
            //         let mut complete_url = String::from(src);
            //         complete_url.remove(0);
            //         let host = String::from("https://www.antbyw.com");
            //         let complete_url = host + &complete_url;
            //         println!(
            //             "complete_url is {}, name is {}",
            //             complete_url,
            //             a_btn.inner_html()
            //         );

            //         if let Some(ref comic_name_temp) = comic_name_2 {
            //             handle_current(
            //                 complete_url,
            //                 ".uk-zjimg img".to_string(),
            //                 "data-src".to_string(),
            //                 format!("./{} {}/{}", *comic_name_temp, text_to_find, a_btn.inner_html()),
            //             )
            //             .await;
            //         }
            //     }
            // }
        }
    }
}

// 从字符串中提取数字
fn extract_number(s: &str) -> usize {
    s.chars()
        .filter_map(|c| c.to_digit(10)) // 过滤出数字字符
        .fold(0, |acc, digit| acc * 10 + digit as usize) // 转换为 usize
}

async fn handle_current(url: String, element_selector: String, attr: String, file: String) {
    let client = Client::new();
    let response = client.get(&url).send().await.unwrap();
    let html_content = response.text().await.unwrap();

    let document = scraper::Html::parse_document(&html_content);
    let image_selector = scraper::Selector::parse(&element_selector).unwrap();
    let images = document.select(&image_selector);
    let mut img_v: Vec<&str> = Vec::new();

    let document_2 = scraper::Html::parse_document(&html_content);
    let image_count_selector = scraper::Selector::parse(".uk-badge.ml8").unwrap();
    let image_count = document_2.select(&image_count_selector).next();
    // let mut count = None;

    for img in images {
        let image = img.value().attr(&attr);

        // println!("{:?}", image);

        match image {
            Some(i) => {
                img_v.push(i);
            }
            None => println!("img is missing!"),
        }
    }

    // count = match image_count {
    //     Some(i) => {
    //         let re = Regex::new(r"\d+").unwrap();
    //         re.find(&i.inner_html()).map(|mat| mat.as_str().to_string())
    //     }
    //     None => Some("".to_string()),
    // };

    // println!("img length is {}", img_v.len(),);
    // println!("img count on page is {}", count.unwrap());
    if let Some(image_count_temp) = image_count {
        println!("image_count is {:?}", image_count_temp.inner_html());
    }

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
    headers.insert(
        HeaderName::from_static("sec-fetch-mode"),
        HeaderValue::from_static("no-cors"),
    );

    println!("domain is {domain}, ext is {ext}");

    let img_format_error = Arc::new(Mutex::new(Vec::new()));

    let semaphore = Arc::new(Semaphore::new(20));
    let mut tasks = vec![];

    let bar = Arc::new(ProgressBar::new(url.len().try_into().unwrap()));
    bar.set_style(ProgressStyle::with_template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg} {duration}")
        .unwrap()
        .progress_chars("##-"));

    for (index, i) in url.iter().enumerate() {
        let img_format_error_clone = Arc::clone(&img_format_error);
        let client = client.clone();
        let headers = headers.clone();
        let file_path = file_path.to_string();
        let semaphore = semaphore.clone();
        let ext = ext.clone();
        let url = i.to_string();
        let bar = Arc::clone(&bar);

        let task = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();

            // println!("downloading {}", url);
            let response = client
                .get(url)
                .headers(headers)
                .send()
                .await
                .unwrap()
                .bytes()
                .await
                .unwrap();

            let name = format!("{}/{}", file_path, index);
            // let path = Path::new(&name);

            let mut img_format = match ext.as_str() {
                "jpg" => image::ImageFormat::Jpeg,
                "png" => image::ImageFormat::Png,
                "webp" => image::ImageFormat::WebP,
                _ => {
                    eprintln!("Error: image extension is unknown!");
                    process::exit(1);
                }
            };

            // let format_result = image::guess_format(&response).unwrap();
            let format_result = match image::guess_format(&response) {
                Ok(format) => {
                    format
                },
                Err(_err) => {
                    let mut img_format_error_clone_lock = img_format_error_clone.lock().unwrap();
                    img_format_error_clone_lock.push(index);
                    // return;
                    img_format
                }
            };

            if format_to_string(&format_result) == "other unknown format" {
                eprintln!("!!!!!!! Unknown image format, index = {}", index);
            }


            if img_format != format_result {
                println!("image ext {} on web is wrong, image library guess_format return {}", format_to_string(&img_format), format_to_string(&format_result));
                img_format = format_result;
            }

            let img = image::load(Cursor::new(&response), img_format);

            match img {
                Ok(img) => {
                    // 将图像转换为 JPG 格式，因为后续转换成pdf的时候，如果是其他图片格式，pdf文件会很大
                    let jpg_bytes = img.to_rgb8(); // 转换为 RGB 格式
                    let mut output_file = File::create(Path::new(&format!("{}.jpg", name))).unwrap();
                    jpg_bytes.write_to(&mut output_file, ImageFormat::Jpeg).unwrap();
                },
                Err(e) => {
                    // this maybe the web image is error, reqwest library can not download it
                    eprintln!(
                        "{} image save is error! ImageError is {} {} is {}",
                        "Error: ".red(),
                        e.to_string().yellow(),
                        "index ".red(),
                        index.to_string().green(),
                    );
                    return;
                    // let mut file = File::create(
                    //     Path::new(&format!("{}.{}", name, ext)),
                    // ).unwrap();
                    // file.write_all(&response).unwrap();
                    // process::exit(1);
                }
            }

            // let mut file = File::create(path).unwrap();
            // file.write_all(&response).unwrap();
            bar.inc(1);
        });

        tasks.push(task);
    }

    for task in tasks {
        let _ = task.await;
    }

    let errors =img_format_error.lock().unwrap();
    if errors.is_empty() {
        bar.finish_with_message(format!("{} is done!", url.len()));
    } else {
        bar.abandon();
        for (i, err) in errors.iter().enumerate() {
            eprintln!(
                "{} {} {} {} image format is unknown",
                "num ".red(),
                (i + 1).to_string().green(),
                "index ".red(),
                (err + 1).to_string().green(),
            );
        }
    }
}

fn format_to_string(format: &ImageFormat) -> &'static str {
    match format {
        ImageFormat::Jpeg => "JPEG",
        ImageFormat::Png => "PNG",
        ImageFormat::Gif => "GIF",
        ImageFormat::Bmp => "BMP",
        ImageFormat::Tiff => "TIFF",
        ImageFormat::WebP => "WEBP",
        ImageFormat::Pnm => "PNM",
        ImageFormat::Tga => "TGA",
        ImageFormat::Dds => "DDS",
        ImageFormat::Ico => "ICO",
        ImageFormat::Hdr => "HDR",
        ImageFormat::OpenExr => "OPENEXR",
        ImageFormat::Farbfeld => "FARBFELD",
        ImageFormat::Avif => "AVIF",
        ImageFormat::Qoi => "QOI",
        ImageFormat::Pcx => "PCX",
        _ => "other unknown format",
    }
}

fn handle_url(url_string: &str) -> String {
    let mut res: String = "".to_string();
    if let Ok(url) = Url::parse(url_string) {
        if let Some(d) = url.domain() {
            // println!("Domain: {}", d);
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
        // println!("File extension: {}", res);
    } else {
        println!("File extension not found in the URL");
    }
    res.to_string()
}
