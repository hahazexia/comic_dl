use clap::{Parser, ValueEnum};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, ORIGIN, REFERER, USER_AGENT};
use reqwest::Client;
use tokio::task;
use std::fs;
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::{Cursor, Write};
use url::Url;
use indicatif::{ProgressBar, ProgressStyle};
use image::{ImageFormat, ImageReader};
use std::sync::{Arc, Mutex};
use std::process;
use std::process::Command;
use tokio::sync::Semaphore;
use colored::Colorize;
use anyhow::{Context, Result};
use std::time::Duration;
use tokio::time::sleep;
use serde_json::json;
// use std::fmt::Write as FmtWrite;

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

#[derive(Debug, Clone, ValueEnum)]
enum DlType {
    Juan,
    Hua,
    Fanwai,
    Current,
    Local,
    Upscale,
}

// const RED: &str = "\x1b[31m";    // 红色
// const GREEN: &str = "\x1b[32m";  // 绿色
// const RESET: &str = "\x1b[0m";   // 重置颜色
// const YELLOW: &str = "\x1b[33m"; // 黄色

// cargo run -- -u "https://www.antbyw.com/plugin.php?id=jameson_manhua&c=index&a=bofang&kuid=147507" -d "juan"
// cargo run -- -u "https://www.antbyw.com/plugin.php?id=jameson_manhua&a=read&kuid=152174&zjid=916038"

const UPSCAYL: &str = "/Applications/Upscayl.app/Contents/Resources/bin/upscayl-bin";
const UPSCAYL_MODEL: &str = "/Applications/Upscayl.app/Contents/Resources/models";

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
        DlType::Local => {
            let _ = handle_local(url).await;
        },
        DlType::Upscale => {
            let _ = handle_upscale(url).await;
        },
    }
}

async fn handle_upscale (url: String) -> Result<bool> {
    let output_path = format!("{url}_upscale");
    let _ = fs::create_dir_all(output_path.to_string().replace(" ", "_"));

    let mut dirs: Vec<serde_json::Value> = Vec::new();

    for entry in fs::read_dir(url)? {
        match entry {
            Ok(dir) => {
                if dir.file_type().unwrap().is_dir() {
                    let dir_name = get_dir_name(PathBuf::from(dir.path().display().to_string())).unwrap();
                    let json_object = json!({
                        "name": dir_name,
                        "path": dir.path().display().to_string(),
                    });
                    dirs.push(json_object);
                }
            },
            Err(_) => {
                eprintln!("{} {}",
                    "Error: ".red(),
                    "read local dir error!".red(),
                );
                process::exit(1);
            },
        };
    }

    dirs.sort_by(|a, b| {
        let a_name = a.get("name").unwrap().as_str().unwrap();
        let b_inner = b.get("name").unwrap().as_str().unwrap();

        // 提取数字并进行比较
        let a_number = extract_number(a_name);
        let b_number = extract_number(b_inner);

        a_number.cmp(&b_number)
    });

    let bar = Arc::new(ProgressBar::new(dirs.len().try_into().unwrap()));
    bar.set_style(ProgressStyle::with_template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg} {duration}")
        .unwrap());

    for dir in dirs.iter() {
        let name = dir.get("name").unwrap().as_str().unwrap();
        let dir_path = dir.get("path").unwrap().as_str().unwrap();

        println!("name is {}", name);

        let new_dir_path = format!("{}/{}", output_path, name);
        let _ = fs::create_dir_all(&new_dir_path);
        // upscayl-bin -i "输入目录" -o "输出目录" -c 50 -m "模型路径" -n "realesrgan-x4plus-anime" -f "png"
        // Usage: upscayl-bin -i infile -o outfile [options]...

        // -h                   show this help
        // -i input-path        input image path (jpg/png/webp) or directory
        // -o output-path       output image path (jpg/png/webp) or directory
        // -z model-scale       scale according to the model (can be 2, 3, 4. default=4)
        // -s output-scale      custom output scale (can be 2, 3, 4. default=4)
        // -r resize            resize output to dimension (default=WxH:default), use '-r help' for more details
        // -w width             resize output to a width (default=W:default), use '-r help' for more details
        // -c compress          compression of the output image, default 0 and varies to 100
        // -t tile-size         tile size (>=32/0=auto, default=0) can be 0,0,0 for multi-gpu
        // -m model-path        folder path to the pre-trained models. default=models
        // -n model-name        model name (default=realesrgan-x4plus, can be realesr-animevideov3 | realesrgan-x4plus-anime | realesrnet-x4plus or any other model)
        // -g gpu-id            gpu device to use (default=auto) can be 0,1,2 for multi-gpu
        // -j load:proc:save    thread count for load/proc/save (default=1:2:2) can be 1:2,2,2:2 for multi-gpu
        // -x                   enable tta mode
        // -f format            output image format (jpg/png/webp, default=ext/png)
        // -v                   verbose output
        let output = Command::new(UPSCAYL)
            .arg("-i")
            .arg(dir_path)
            .arg("-o")
            .arg(&new_dir_path)
            .arg("-s")
            .arg("4")
            .arg("-c")
            .arg("50")
            .arg("-m")
            .arg(UPSCAYL_MODEL)
            .arg("-n")
            .arg("realesrgan-x4fast")
            .arg("-f")
            .arg("jpg")
            .output()
            .expect("Failed to execute command");

        // 处理输出
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            println!("Output: {}", stdout);
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("Error: {}", stderr);
        }

        bar.inc(1);
    }
    let finish_text = format!("{} is done!", dirs.len());
    bar.finish_with_message(finish_text.bright_blue().to_string());

    Ok(true)
}

async fn handle_local (url: String) -> Result<bool>{
    let output_path = format!("{url}_jpg");
    let _ = fs::create_dir_all(output_path.to_string().replace(" ", "_"));

    let mut dirs: Vec<serde_json::Value> = Vec::new();

    for entry in fs::read_dir(url)? {
        match entry {
            Ok(dir) => {
                if dir.file_type().unwrap().is_dir() {
                    let dir_name = get_dir_name(PathBuf::from(dir.path().display().to_string())).unwrap();
                    let json_object = json!({
                        "name": dir_name,
                        "path": dir.path().display().to_string(),
                    });
                    dirs.push(json_object);
                }
            },
            Err(_) => {
                eprintln!("{} {}",
                    "Error: ".red(),
                    "read local dir error!".red(),
                );
                process::exit(1);
            },
        };
    }

    dirs.sort_by(|a, b| {
        let a_name = a.get("name").unwrap().as_str().unwrap();
        let b_inner = b.get("name").unwrap().as_str().unwrap();

        // 提取数字并进行比较
        let a_number = extract_number(a_name);
        let b_number = extract_number(b_inner);

        a_number.cmp(&b_number)
    });

    for dir in dirs.iter() {
        let name = dir.get("name").unwrap().as_str().unwrap();
        let dir_path = dir.get("path").unwrap().as_str().unwrap();

        println!("name is {}", name);

        let new_dir_path = format!("{}/{}", output_path, name);
        let new_dir_path_clone = Arc::new(format!("{}/{}", output_path, name));
        let _ = fs::create_dir_all(new_dir_path);

        let mut files: Vec<_> = fs::read_dir(dir_path).unwrap().filter_map(Result::ok).collect();

        files.sort_by(|a, b| {
            let binding = a.file_name();
            let a_name = binding.to_str().unwrap();
            let binding = b.file_name();
            let b_name = binding.to_str().unwrap();

            // 提取数字并进行比较
            let a_number = extract_number(a_name);
            let b_number = extract_number(b_name);

            a_number.cmp(&b_number)
        });

        let bar = Arc::new(ProgressBar::new(files.len().try_into().unwrap()));
        bar.set_style(ProgressStyle::with_template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg} {duration}")
            .unwrap());

        let semaphore = Arc::new(Semaphore::new(20));
        let mut tasks = vec![];
        let entries = fs::read_dir(dir_path).unwrap();

        for entry in entries {
            match entry {
                Ok(img) => {
                    let path = img.path();
                    let permit = semaphore.clone().acquire_owned().await.unwrap(); // 获取许可

                    let new_dir_path_clone_arc = Arc::clone(&new_dir_path_clone);
                    let bar_clone_arc = Arc::clone(&bar);
                    let task = task::spawn(async move {
                        let _permit = permit;
                        if path.is_file() && is_image_file(&path) {
                            let img_name = get_file_name_without_extension(&path).unwrap();
                            let temp_img = ImageReader::open(&path).unwrap().decode().unwrap();
                            let _ = temp_img.save(format!("{}/{}.jpg", new_dir_path_clone_arc, extract_number(&img_name)));
                            bar_clone_arc.inc(1);
                        }
                    });

                    tasks.push(task);
                },
                Err(_) => {
                    bar.abandon();
                    eprintln!("{} {}",
                        "Error: ".red(),
                        "read local dir image error!".red(),
                    );
                    process::exit(1);
                },
            }
        }

        for task in tasks {
            let _ = task.await;
        }

        let finish_text = format!("{} is done!", files.len());
        bar.finish_with_message(finish_text.bright_blue().to_string());
    }

    Ok(true)
}

fn get_dir_name<P: AsRef<Path>>(path: P) -> Option<String> {
    let path = path.as_ref();
    path.file_name().and_then(|name| name.to_str().map(|s| s.to_string()))
}
fn is_image_file(path: &Path) -> bool {
    match path.extension().and_then(|s| s.to_str()) {
        Some(ext) => matches!(ext.to_lowercase().as_str(), "jpg" | "jpeg" | "png" | "gif" | "bmp" | "tiff" | "webp"),
        None => false,
    }
}
fn get_file_name_without_extension(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

// async fn handle_juan_hua_fanwai(url: String, dl_type: DlType, min: f32, max: f32) {
async fn handle_juan_hua_fanwai(url: String, dl_type: DlType) {
    // current only support antbyw.com
    if !url.contains("https://www.antbyw.com/") {
        eprintln!("Error: current only support antbyw.com.");
        process::exit(1);
    } else {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build().unwrap();
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
            DlType::Current => "",
            DlType::Juan => "单行本",
            DlType::Hua => "单话",
            DlType::Fanwai => "番外篇",
            DlType::Local => "_",
            DlType::Upscale => "_",
        };

        if let Some(name) = comic_name {
            println!("{}{}", "comic name is ".bright_yellow(), name.to_string().bright_green());
            // create juan output directory
            let _ = fs::create_dir_all(format!("./{}_{}", &name, text_to_find).replace(" ", "_"));
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

            // let filtered_target: Vec<_> = target.iter()
            //     .filter(|&&x| {
            //         let x_inner = x.inner_html();
            //         let x_number = extract_number(&x_inner);

            //         (x_number as f32) > min && (x_number as f32) < max
            //     })
            //     .cloned()
            //     .collect();

            for (i, a_btn) in target.iter().enumerate() {
                if let Some(src) = a_btn.value().attr("href") {
                    // println!("src is {}, inner is {}", src, a_btn.inner_html());
                    let mut complete_url = String::from(src);
                    complete_url.remove(0);
                    let host = String::from("https://www.antbyw.com");
                    let complete_url = host + &complete_url;
                    println!(
                        "{} {} {}is {}, {}is {}",
                        "num".red(),
                        format!("{}", i + 1).red(),
                        "complete_url ".purple(),
                        complete_url,
                        "name ".purple(),
                        a_btn.inner_html()
                    );


                    if let Some(ref comic_name_temp) = comic_name_2 {
                        let dir_path = format!("./{}_{}/{}", *comic_name_temp, text_to_find, a_btn.inner_html());
                        let path = Path::new(&dir_path);

                        println!("{}", dir_path.to_string().bright_black().on_bright_white());

                        if path.is_dir() {
                            println!("{}: {}", "dir already exist, continue next".green(), dir_path);
                            continue;
                        } else {
                            let max_retries = 3; // 最大重试次数
                            let mut attempts = 0;
                            loop {
                                attempts += 1;

                                match handle_current(
                                    complete_url.clone(),
                                    ".uk-zjimg img".to_string(),
                                    "data-src".to_string(),
                                    format!("./{}_{}/{}", *comic_name_temp, text_to_find, a_btn.inner_html()),
                                )
                                .await {
                                    Ok(_) => {
                                        println!();
                                        break;
                                    },
                                    Err(e) => {
                                        if attempts < max_retries {
                                            println!("{} Attempt {}/{} failed: {}. Retrying...", "Error: ".red(), attempts, max_retries, e);
                                            sleep(Duration::from_secs(2)).await; // 等待 2 秒后重试
                                            continue;
                                        } else {
                                            eprintln!("{} All {} attempts failed: {}", "Error: ".red(), max_retries, e);
                                            process::exit(1);
                                        }
                                    },
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// 从字符串中提取数字
fn extract_number(s: &str) -> usize {
    s.chars()
        .filter_map(|c| c.to_digit(10)) // 过滤出数字字符
        .fold(0, |acc, digit| acc * 10 + digit as usize) // 转换为 usize
}

async fn handle_current(url: String, element_selector: String, attr: String, file: String) -> Result<()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    let response = client.get(&url).send().await.context("Failed to send request".red())?;
    let html_content = response.text().await.context("Failed to get response text".red())?;

    let document = scraper::Html::parse_document(&html_content);
    let image_selector = scraper::Selector::parse(&element_selector)
        .map_err(|e| anyhow::anyhow!("Failed to parse image selector: {:?}", e))?;
    let images = document.select(&image_selector);
    let mut img_v: Vec<&str> = Vec::new();

    let document_2 = scraper::Html::parse_document(&html_content);
    let image_count_selector = scraper::Selector::parse(".uk-badge.ml8")
        .map_err(|e| anyhow::anyhow!("Failed to parse image count selector: {:?}", e))?;
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
        println!("{}{:?}", "image_count is ".bright_red(), image_count_temp.inner_html());
    }

    down_img(img_v, &format!("./{}", &file)).await;
    Ok(())
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
    bar.set_style(ProgressStyle::with_template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg} {duration}")
        .unwrap());
    // bar.set_style(ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})")
    //     .unwrap()
    //     .with_key("eta", |state: &ProgressState, w: &mut dyn FmtWrite| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap()));

        // .progress_chars("##-"));

    for (index, i) in url.iter().enumerate() {
        let img_format_error_clone = Arc::clone(&img_format_error);
        let client = client.clone();
        let headers = headers.clone();
        let file_path = file_path.to_string();
        let semaphore = semaphore.clone();
        let ext = ext.clone();
        let temp_url = i.to_string();
        let bar = Arc::clone(&bar);

        let task = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();

            // println!("downloading {}", temp_url);
            let response = client
                .get(temp_url)
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
                    // althrough image download failed, still save the damaged image as a placeholder, for replacing it after all is done
                    let mut file = File::create(
                        Path::new(&format!("{}.{}", name, ext)),
                    ).unwrap();
                    file.write_all(&response).unwrap();
                    return;
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
        let finish_text = format!("{} is done!", url.len());

        bar.finish_with_message(finish_text.bright_blue().to_string());
    } else {
        bar.abandon();
        for (i, err) in errors.iter().enumerate() {
            eprintln!(
                "{} {} {} {} image format is unknown",
                "num ".red(),
                (i + 1).to_string().bright_yellow(),
                "index ".red(),
                (err + 1).to_string().bright_yellow(),
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
