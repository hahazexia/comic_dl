use bytes::Bytes;
use image::ImageFormat;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, ORIGIN, REFERER, USER_AGENT};
use tokio::sync::Semaphore;
use tokio::time::timeout;
use std::fs::File;
use std::io::{Cursor, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::{fs, process};
use std::{collections::HashMap, time::Duration};
use anyhow::{Context, Result};
use colored::Colorize;
use serde::Deserialize;

use crate::utils::{format_to_string, handle_img_extension, handle_url};


/**
 * Aggregate response
 * {
    "result": "ok",
    "volumes": {
        "1": {
            "volume": "1",
            "count": 11,
            "chapters": {
                "2": {
                    "chapter": "2",
                    "id": "65f8c566-acc9-4acc-8d43-eca95ddda001",
                    "others": [
                        "0b62b078-71a0-4385-b904-589fc8ee064b"
                    ],
                    "count": 2
                }
            }
        }
    }
}
*/
#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct Aggregate {
    result: String,
    volumes: std::collections::HashMap<String, Volume>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct Volume {
    volume: String,
    count: u32,
    chapters: std::collections::HashMap<String, Chapter>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct Chapter {
    chapter: String,
    id: String,
    others: Vec<String>,
    count: u32,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct SerialHashmap {
    volume: String,
    chapter: String,
}


/**
 * {
    "result": "ok",
    "baseUrl": "https:\/\/cmdxd98sb0x3yprd.mangadex.network",
    "chapter": {
        "hash": "3541196eaeb8a67e9b801a152c24c161",
        "data": [
            "1-3bd7d1a9fd25d13a3d1d50f95536eb5463f93419b161b4512e89781a6f1ad3fa.png",
        ],
        "dataSaver": [
            "1-0fee13609ee90f2f6b5203eee5cf91d865b0aa287c7a26a384b91bcc717b89ab.jpg",
        ]
    }
}
 */
#[derive(Deserialize, Debug)]
#[allow(dead_code)]
#[serde(rename_all = "camelCase")]
struct ImageRes {
    result: String,
    base_url: String,
    chapter: ImageResChapter,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
#[serde(rename_all = "camelCase")]
struct ImageResChapter {
    hash: String,
    data: Vec<String>,
    data_saver: Vec<String>,
}

pub async fn handle_mangadex(url: String) -> Result<()> {
    let url_split_vec: Vec<&str> = url.split("/").collect();
    let comic_id = url_split_vec[url_split_vec.len() - 2];
    let comic_name = url_split_vec[url_split_vec.len() - 1];
    let comic_detail_url = format!("https://api.mangadex.org/manga/{}/aggregate?translatedLanguage[]=en", comic_id);
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36"));
    headers.insert(REFERER, HeaderValue::from_str("https://mangadex.org").unwrap());
    headers.insert(ORIGIN, HeaderValue::from_str("https://mangadex.org").unwrap());

    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    let response = client.get(&comic_detail_url).headers(headers).send().await?.text().await.context("Failed to send request".red())?;
    // dbg!(&response);
    let source:Aggregate = serde_json::from_str(&response)?;
    let mut url_vec:Vec<String> = Vec::new();
    let mut serial_hashmap: HashMap<String, SerialHashmap> = HashMap::new();

    let volumes = source.volumes;
    for volume in volumes.keys() {
        let volume_info = &volumes[volume];
        let chapters = &volume_info.chapters;
        for chapter in chapters.keys() {
            let chapter = &chapters[chapter];
            let chapter_url = format!("https://mangadex.org/chapter/{}/{}", &chapter.id, &chapter.chapter);
            url_vec.push(chapter_url.clone());
            serial_hashmap.insert(chapter_url, SerialHashmap {
                volume: volume.to_string(),
                chapter: chapter.chapter.clone(),
            });
            if chapter.others.len() > 0 {
                for (index, other) in chapter.others.iter().enumerate() {
                    let chapter_url = format!("https://mangadex.org/chapter/{}/{}", &other, &chapter.chapter);
                    url_vec.push(chapter_url.clone());
                    serial_hashmap.insert(chapter_url, SerialHashmap{
                        volume: volume.to_string(),
                        chapter: format!("{}_other_{}", chapter.chapter.clone(), index),
                    });
                }
            }
        }
    }

    url_vec.sort_by(|a, b| {
        let a_info = serial_hashmap.get(a).unwrap();
        let b_info = serial_hashmap.get(b).unwrap();
        let a_volume = a_info.volume.parse::<i32>().unwrap_or_default();
        let a_chapter = a_info.chapter.parse::<i32>().unwrap_or_default();
        let b_volume = b_info.volume.parse::<i32>().unwrap_or_default();
        let b_chapter = b_info.chapter.parse::<i32>().unwrap_or_default();
        a_volume.cmp(&b_volume).then_with(|| a_chapter.cmp(&b_chapter))
    });

    println!("{}{}", "comic name is ".bright_yellow(), comic_name.bright_green());

    for chapter in url_vec.iter() {
        handle_mangadex_chapter(chapter.to_string(), &serial_hashmap, comic_name.to_string()).await;
    }

    Ok(())
}


async fn handle_mangadex_chapter (chapter_url: String, serial_hashmap: &HashMap<String, SerialHashmap>, comic_name: String) {
    let url_split_vec: Vec<&str> = chapter_url.split("/").collect();
    let chapter_id = if url_split_vec.len() > 5 { url_split_vec[url_split_vec.len() - 2] } else { url_split_vec[url_split_vec.len() - 1] };
    let mut urls: Vec<String> = Vec::new();
    let api_img = format!("https://api.mangadex.org/at-home/server/{}?forcePort443=false", chapter_id);
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36"));
    headers.insert(REFERER, HeaderValue::from_str("https://mangadex.org").unwrap());
    headers.insert(ORIGIN, HeaderValue::from_str("https://mangadex.org").unwrap());

    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build().unwrap();

    let img_list_res = client.get(api_img).headers(headers).send().await.unwrap().text().await.unwrap();
    let source: ImageRes = serde_json::from_str(&img_list_res).unwrap();

    let base_url = source.base_url;
    let base_hash = source.chapter.hash;
    for (_index, img) in source.chapter.data.iter().enumerate() {
        let temp = format!("{}/data/{}/{}", base_url, base_hash, img);
        urls.push(temp);
    }

    let chapter_info = serial_hashmap.get(&chapter_url).unwrap();
    let chapter_local_path = format!("./{}/volume{}_chapter{}", comic_name, &chapter_info.volume, &chapter_info.chapter);
    // let _ = fs::create_dir_all(&chapter_local_path);

    println!("{}{} {}{}", "volume: ".bright_yellow(), &chapter_info.volume.bright_green(), "chapter: ".bright_yellow(), &chapter_info.chapter.bright_green());

    down_img(urls, &chapter_local_path).await;

}


pub async fn down_img(url: Vec<String>, file_path: &str) {
    let _ = fs::create_dir_all(&file_path);
    let client = Client::new();
    let _domain = handle_url(&url[0]);
    let ext = handle_img_extension(&url[0]);
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36"));
    headers.insert(REFERER, HeaderValue::from_str("https://mangadex.org").unwrap());
    headers.insert(ORIGIN, HeaderValue::from_str("https://mangadex.org").unwrap());
    headers.insert(
        HeaderName::from_static("sec-fetch-mode"),
        HeaderValue::from_static("no-cors"),
    );

    // println!("domain is {domain}, ext is {ext}");

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
                    Duration::from_secs(5),
                    client.get(&temp_url).headers(headers.clone()).send()
                ).await;
                // dbg!(&response_result);

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
                        println!("{}", _e);
                        res = Bytes::from("");
                        if let Some(msg_indx) = messages.get(1) {
                            *err_counts.entry(msg_indx).or_insert(0) += 1;
                        }
                        // eprintln!("请求错误: {}", _e);
                    }
                    Err(_e) => {
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

            let name = format!("{}/{}", &file_path, index);
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