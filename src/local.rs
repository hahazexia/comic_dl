
use std::{fs, path::{Path, PathBuf}, process::Command};
use serde_json::json;
use colored::Colorize;
use std::process;
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task;
use image::ImageReader;

use crate::utils::{
    get_dir_name,
    is_image_file,
    get_file_name_without_extension,
    extract_number,
};

const _UPSCAYL_MAC: &str = "/Applications/Upscayl.app/Contents/Resources/bin/upscayl-bin";
const _UPSCAYL_MODEL_MAC: &str = "/Applications/Upscayl.app/Contents/Resources/models";

const _UPSCAYL_WIN: &str = "D:/upscayl/resources/bin/upscayl-bin";
const _UPSCAYL_MODEL_WIN: &str = "D:/upscayl/resources/models";

// 单个图片排队upscale
pub async fn handle_upscale (url: String) -> Result<bool, String> {
    let output_path = format!("{url}_upscale");
    let _ = fs::create_dir_all(output_path.to_string().replace(" ", "_"));

    let mut dirs: Vec<serde_json::Value> = Vec::new();

    for entry in fs::read_dir(url).map_err(|e| e.to_string())? {
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
        // let new_dir_path_obj = Path::new(&new_dir_path);
        // if new_dir_path_obj.is_dir() {
        //     println!("{}: {}", "dir already exist, continue next".green(), &new_dir_path);
        //     bar.inc(1);
        //     continue;
        // }
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
        let upscayl;
        let upscayl_model;

        #[cfg(target_os = "windows")]
        {
            upscayl = _UPSCAYL_WIN;
            upscayl_model = _UPSCAYL_MODEL_WIN;
        }
        #[cfg(target_os = "macos")]
        {
            upscayl = _UPSCAYL_MAC;
            upscayl_model = _UPSCAYL_MODEL_MAC;
        }

        let mut image_files: Vec<PathBuf> = Vec::new();
        for entry in fs::read_dir(dir_path).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();

            if path.is_file() && path.extension().map_or(false, |ext| ext == "jpg") {
                image_files.push(path);
            }
        }

        image_files.sort_by_key(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("")
                .trim_end_matches(".jpg")
                .parse::<u32>()
                .unwrap_or(0)
        });


        let bar = Arc::new(ProgressBar::new(image_files.len().try_into().unwrap()));
        bar.set_style(ProgressStyle::with_template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg} {duration}")
            .unwrap());

        for file in &image_files {
            if let Some(temp_img_path) = file.to_str() {
                if let Some(img_name) = file.file_name() {
                    let output_img = format!("{}/{}", &new_dir_path, img_name.to_str().unwrap());
                    if Path::new(&output_img).exists() {
                        bar.inc(1);
                        continue;
                    }
                    let _output = Command::new(upscayl)
                        .arg("-i")
                        .arg(&temp_img_path)
                        .arg("-o")
                        .arg(output_img)
                        .arg("-s")
                        .arg("2")
                        .arg("-c")
                        .arg("50")
                        .arg("-m")
                        .arg(upscayl_model)
                        .arg("-n")
                        .arg("4x-DWTP-ds-esrgan-5")
                        .arg("-j")
                        .arg("1:1:1")
                        .arg("-f")
                        .arg("jpg")
                        .output()
                        .expect("Failed to execute command");

                    // if output.status.success() {``
                    //     let stdout = String::from_utf8_lossy(&output.stdout);
                    //     println!("Output: {}", stdout);
                    // } else {
                    //     let stderr = String::from_utf8_lossy(&output.stderr);
                    //     eprintln!("Error: {}", stderr);
                    // }
                    bar.inc(1);
                    // thread::sleep(Duration::from_secs(2));
                }
            }
        }
        let finish_text = format!("{} is done!", dirs.len());
        bar.finish_with_message(finish_text.bright_blue().to_string());
        // let output = Command::new(upscayl)
        //     .arg("-i")
        //     .arg(dir_path)
        //     .arg("-o")
        //     .arg(&new_dir_path)
        //     .arg("-s")
        //     .arg("2")
        //     .arg("-c")
        //     .arg("50")
        //     .arg("-m")
        //     .arg(upscayl_model)
        //     .arg("-n")
        //     .arg("4x-DWTP-ds-esrgan-5")
        //     .arg("-j")
        //     .arg("1:1:1")
        //     .arg("-f")
        //     .arg("jpg")
        //     .output()
        //     .expect("Failed to execute command");

        // thread::sleep(Duration::from_secs(30));
    }

    Ok(true)
}

pub async fn handle_local (url: String) -> Result<bool, String>{
    let output_path = format!("{url}_jpg");
    let _ = fs::create_dir_all(output_path.to_string().replace(" ", "_"));

    let mut dirs: Vec<serde_json::Value> = Vec::new();

    for entry in fs::read_dir(url).map_err(|e| e.to_string())? {
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



// 批量目录upscale
// async fn handle_upscale (url: String) -> Result<bool> {
//     let output_path = format!("{url}_upscale");
//     let _ = fs::create_dir_all(output_path.to_string().replace(" ", "_"));

//     let mut dirs: Vec<serde_json::Value> = Vec::new();

//     for entry in fs::read_dir(url)? {
//         match entry {
//             Ok(dir) => {
//                 if dir.file_type().unwrap().is_dir() {
//                     let dir_name = get_dir_name(PathBuf::from(dir.path().display().to_string())).unwrap();
//                     let json_object = json!({
//                         "name": dir_name,
//                         "path": dir.path().display().to_string(),
//                     });
//                     dirs.push(json_object);
//                 }
//             },
//             Err(_) => {
//                 eprintln!("{} {}",
//                     "Error: ".red(),
//                     "read local dir error!".red(),
//                 );
//                 process::exit(1);
//             },
//         };
//     }

//     dirs.sort_by(|a, b| {
//         let a_name = a.get("name").unwrap().as_str().unwrap();
//         let b_inner = b.get("name").unwrap().as_str().unwrap();

//         // 提取数字并进行比较
//         let a_number = extract_number(a_name);
//         let b_number = extract_number(b_inner);

//         a_number.cmp(&b_number)
//     });

//     let bar = Arc::new(ProgressBar::new(dirs.len().try_into().unwrap()));
//     bar.set_style(ProgressStyle::with_template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg} {duration}")
//         .unwrap());

//     for dir in dirs.iter() {
//         let name = dir.get("name").unwrap().as_str().unwrap();
//         let dir_path = dir.get("path").unwrap().as_str().unwrap();

//         println!("name is {}", name);

//         let new_dir_path = format!("{}/{}", output_path, name);
//         let new_dir_path_obj = Path::new(&new_dir_path);
//         if new_dir_path_obj.is_dir() {
//             println!("{}: {}", "dir already exist, continue next".green(), &new_dir_path);
//             bar.inc(1);
//             continue;
//         }
//         let _ = fs::create_dir_all(&new_dir_path);
//         // upscayl-bin -i "输入目录" -o "输出目录" -c 50 -m "模型路径" -n "realesrgan-x4plus-anime" -f "png"
//         // Usage: upscayl-bin -i infile -o outfile [options]...

//         // -h                   show this help
//         // -i input-path        input image path (jpg/png/webp) or directory
//         // -o output-path       output image path (jpg/png/webp) or directory
//         // -z model-scale       scale according to the model (can be 2, 3, 4. default=4)
//         // -s output-scale      custom output scale (can be 2, 3, 4. default=4)
//         // -r resize            resize output to dimension (default=WxH:default), use '-r help' for more details
//         // -w width             resize output to a width (default=W:default), use '-r help' for more details
//         // -c compress          compression of the output image, default 0 and varies to 100
//         // -t tile-size         tile size (>=32/0=auto, default=0) can be 0,0,0 for multi-gpu
//         // -m model-path        folder path to the pre-trained models. default=models
//         // -n model-name        model name (default=realesrgan-x4plus, can be realesr-animevideov3 | realesrgan-x4plus-anime | realesrnet-x4plus or any other model)
//         // -g gpu-id            gpu device to use (default=auto) can be 0,1,2 for multi-gpu
//         // -j load:proc:save    thread count for load/proc/save (default=1:2:2) can be 1:2,2,2:2 for multi-gpu
//         // -x                   enable tta mode
//         // -f format            output image format (jpg/png/webp, default=ext/png)
//         // -v                   verbose output
//         let upscayl;
//         let upscayl_model;

//         #[cfg(target_os = "windows")]
//         {
//             upscayl = _UPSCAYL_WIN;
//             upscayl_model = _UPSCAYL_MODEL_WIN;
//         }
//         #[cfg(target_os = "macos")]
//         {
//             upscayl = _UPSCAYL_MAC;
//             upscayl_model = _UPSCAYL_MODEL_MAC;
//         }

//         let output = Command::new(upscayl)
//             .arg("-i")
//             .arg(dir_path)
//             .arg("-o")
//             .arg(&new_dir_path)
//             .arg("-s")
//             .arg("2")
//             .arg("-c")
//             .arg("50")
//             .arg("-m")
//             .arg(upscayl_model)
//             .arg("-n")
//             .arg("4x-DWTP-ds-esrgan-5")
//             .arg("-j")
//             .arg("1:1:1")
//             .arg("-f")
//             .arg("jpg")
//             .output()
//             .expect("Failed to execute command");

//         // 处理输出
//         if output.status.success() {
//             let stdout = String::from_utf8_lossy(&output.stdout);
//             println!("Output: {}", stdout);
//         } else {
//             let stderr = String::from_utf8_lossy(&output.stderr);
//             eprintln!("Error: {}", stderr);
//         }

//         bar.inc(1);
//         thread::sleep(Duration::from_secs(30));
//     }
//     let finish_text = format!("{} is done!", dirs.len());
//     bar.finish_with_message(finish_text.bright_blue().to_string());

//     Ok(true)
// }