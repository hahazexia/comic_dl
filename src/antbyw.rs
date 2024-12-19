use reqwest::header::{HeaderMap, HeaderName, HeaderValue, ORIGIN, REFERER, USER_AGENT};
use reqwest::Client;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::fs::File;
use std::io::{Cursor, Write};
use indicatif::{ProgressBar, ProgressStyle};
use image::ImageFormat;
use std::sync::{Arc, Mutex};
use std::process;
use tokio::sync::Semaphore;
use colored::Colorize;
use anyhow::{Context, Result};
use std::time::Duration;
use tokio::time::sleep;
use tokio::time::timeout;
use bytes::Bytes;

use crate::utils::{
    format_to_string,
    handle_url,
    handle_img_extension,
    extract_number,
};

use crate::dl_type::DlType;

pub async fn handle_juan_hua_fanwai(url: String, dl_type: DlType) {
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
            println!("{}{}", "comic name is ".yellow(), name.to_string().bright_green());
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

            for (i, a_btn) in target.iter().enumerate() {
                if let Some(src) = a_btn.value().attr("href") {
                    // println!("src is {}, inner is {}", src, a_btn.inner_html());
                    let mut complete_url = String::from(src);
                    complete_url.remove(0);
                    let host = String::from("https://www.antbyw.com");
                    let complete_url = host + &complete_url;
                    println!(
                        "{} {} {}is {}, {}is {}",
                        "num".yellow(),
                        format!("{}", i + 1).yellow(),
                        "complete_url ".purple(),
                        complete_url,
                        "name ".purple(),
                        a_btn.inner_html()
                    );


                    if let Some(ref comic_name_temp) = comic_name_2 {
                        let dir_path = format!("./{}_{}/{}", *comic_name_temp, text_to_find, a_btn.inner_html());
                        // let path = Path::new(&dir_path);

                        println!("{}", dir_path.to_string().bright_black().on_bright_white());

                        // if path.is_dir() {
                        //     println!("{}: {}", "dir already exist, continue next".green(), dir_path);
                        //     continue;
                        // } else {}
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

pub async fn handle_current(url: String, element_selector: String, attr: String, file: String) -> Result<()> {
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
        println!("{}{:?}", "image_count is ".yellow(), image_count_temp.inner_html());
    }

    down_img(img_v, &format!("./{}", &file)).await;
    Ok(())
}

pub async fn down_img(url: Vec<&str>, file_path: &str) {
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

            let name = format!("{}/{}.jpg", &file_path, &index);
            if Path::new(&name).exists() {
                // println!("{} jpg is already exist, next",
                //     index.to_string().green(),
                // );
                bar.inc(1);
                return;
            }

            let mut res;
            let mut count = 0;
            let messages = vec![
                "请求失败，状态码",
                "请求错误",
                "请求超时",
                "字节转换失败",
            ];
            let mut err_counts: HashMap<&str, usize> = HashMap::new();
            loop {
                count += 1;
                let response_result = timeout(
                    Duration::from_secs(20),
                    client.get(&temp_url).headers(headers.clone()).send()
                ).await;

                match response_result {
                    Ok(Ok(response)) => {
                        if response.status().is_success() {
                            let res_temp = response.bytes().await;

                            match res_temp {
                                Ok(bytes) => {
                                    res = bytes;
                                }
                                Err(_e) => {
                                    res = Bytes::from("");
                                    // eprintln!("bytes error is {:?}", e);
                                    if let Some(msg_indx) = messages.get(3) {
                                        *err_counts.entry(msg_indx).or_insert(3) += 1;
                                    }
                                }
                            }
                            // 在这里处理获取到的字节，例如保存到文件
                            // println!("成功获取图片，大小: {} bytes", res.len());
                            break; // 成功后退出循环
                        } else {
                            res = Bytes::from("");
                            if let Some(msg_indx) = messages.get(0) {
                                *err_counts.entry(msg_indx).or_insert(0) += 1;
                            }
                            // eprintln!("请求失败，状态码: {}", response.status());
                        }
                    }
                    Ok(Err(_e)) => {
                        res = Bytes::from("");
                        if let Some(msg_indx) = messages.get(1) {
                            *err_counts.entry(msg_indx).or_insert(0) += 1;
                        }
                        // eprintln!("请求错误: {}", _e);
                    }
                    Err(_) => {
                        res = Bytes::from("");
                        if let Some(msg_indx) = messages.get(2) {
                            *err_counts.entry(msg_indx).or_insert(0) += 1;
                        }
                        // eprintln!("请求超时");
                    }
                }

                if count > 10 {
                    break;
                }

                tokio::time::sleep(Duration::from_secs(1)).await;
            }

            if res.len() == 0 {
                eprintln!("attempt {} times, but failed, url is {}, index is {}", count, &temp_url, &index);
                for (msg, index) in err_counts {
                    println!("{}: {} 次", msg.red(), index.to_string().yellow());
                }
                let mut img_format_error_clone_lock = img_format_error_clone.lock().unwrap();
                img_format_error_clone_lock.push(index);
                return;
            }

            // println!("downloading {}", temp_url);
            // let response = client
            //     .get(&temp_url)
            //     .headers(headers)
            //     .send()
            //     .await
            //     .unwrap()
            //     .bytes()
            //     .await
            //     .unwrap();

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
            let format_result = match image::guess_format(&res) {
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

            // println!("format_result is {:?}", &format_result);

            if format_to_string(&format_result) == "other unknown format" {
                eprintln!("!!!!!!! Unknown image format, index = {}", index);
            }


            if img_format != format_result {
                println!("image ext {} on web is wrong, image library guess_format return {}", format_to_string(&img_format), format_to_string(&format_result));
                img_format = format_result;
            }

            let img = image::load(Cursor::new(&res), img_format);

            match img {
                Ok(img) => {
                    // 将图像转换为 JPG 格式，因为后续转换成pdf的时候，如果是其他图片格式，pdf文件会很大
                    let jpg_bytes = img.to_rgb8(); // 转换为 RGB 格式
                    let mut output_file = File::create(Path::new(&format!("{}.jpg", name))).unwrap();
                    jpg_bytes.write_to(&mut output_file, ImageFormat::Jpeg).unwrap();
                    bar.inc(1);
                },
                Err(e) => {
                    // this maybe the web image is error, reqwest library can not download it
                    eprintln!(
                        "{} image save is error! ImageError is {} {} is {} url is {}",
                        "Error: ".red(),
                        e.to_string().yellow(),
                        "index ".red(),
                        index.to_string().green(),
                        &temp_url,
                    );
                    // althrough image download failed, still save the damaged image as a placeholder, for replacing it after all is done
                    let mut file = File::create(
                        Path::new(&format!("{}.{}", name, ext)),
                    ).unwrap();
                    file.write_all(&res).unwrap();
                    return;
                    // process::exit(1);
                }
            }

            // let mut file = File::create(path).unwrap();
            // file.write_all(&response).unwrap();
            // bar.inc(1);
        });

        tasks.push(task);
    }

    for task in tasks {
        let _ = task.await;
    }

    let errors = img_format_error.lock().unwrap();
    if errors.is_empty() {
        let finish_text = format!("{} is done!", url.len());

        bar.finish_with_message(finish_text.bright_blue().to_string());
    } else {
        bar.abandon();
        for (i, err) in errors.iter().enumerate() {
            eprintln!(
                "{} {} {} {} image format is unknown",
                "num ".red(),
                (i + 1).to_string().yellow(),
                "index ".red(),
                (err + 1).to_string().yellow(),
            );
        }
    }
}

