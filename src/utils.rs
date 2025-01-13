use image::ImageFormat;
use url::Url;
use std::{fs, path::Path};
use std::io::Error;

// 根据image库类型返回图片格式字符串
pub fn format_to_string(format: &ImageFormat) -> &'static str {
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

// 处理图片url
pub fn handle_url(url_string: &str) -> String {
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

// 字符串转数组
pub fn split_string(input: &str, delimiter: &str) -> Vec<String> {
    input.split(delimiter).map(|s| s.to_string()).collect()
}

// 数组元素连接成字符串
pub fn join_strings(strings: Vec<&str>, delimiter: &str) -> String {
    strings.join(delimiter)
}

// 从图片url中获取图片扩展名
pub fn handle_img_extension(url_string: &str) -> String {
    let mut res = "";
    if let Some(index) = url_string.rfind('.') {
        res = &url_string[index + 1..];
        // println!("File extension: {}", res);
    } else {
        println!("File extension not found in the URL");
    }
    res.to_string()
}

// 从字符串中提取数字
pub fn extract_number(s: &str) -> usize {
    s.chars()
        .filter_map(|c| c.to_digit(10)) // 过滤出数字字符
        .fold(0, |acc, digit| acc * 10 + digit as usize) // 转换为 usize
}

// 获取本地目录的名称
pub fn get_dir_name<P: AsRef<Path>>(path: P) -> Option<String> {
    let path = path.as_ref();
    path.file_name().and_then(|name| name.to_str().map(|s| s.to_string()))
}

// 判断一个路径是否是图片文件
pub fn is_image_file(path: &Path) -> bool {
    match path.extension().and_then(|s| s.to_str()) {
        Some(ext) => matches!(ext.to_lowercase().as_str(), "jpg" | "jpeg" | "png" | "gif" | "bmp" | "tiff" | "webp"),
        None => false,
    }
}

// 获取不带扩展名的文件名
pub fn get_file_name_without_extension(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

// 获取二级域名
pub fn get_second_level_domain(url_str: &str) -> Option<String> {
    let url = Url::parse(url_str).ok()?;

    let host = url.host_str()?;

    let parts: Vec<&str> = host.split('.').collect();

    if parts.len() >= 2 {
        Some(parts[parts.len() - 2].to_string())
    } else {
        None
    }
}

pub fn create_file_if_not_exists(file_name: &str) -> std::io::Result<()> {
    // 检查文件是否存在
    if !Path::new(&file_name).exists() {
        // 如果文件不存在，创建目录
        fs::create_dir_all(Path::new(&file_name).parent().unwrap())?;

        // 创建文件
        fs::File::create(&file_name)?;
        // println!("File created: {}", file_name);
    } else {
        // println!("File already exists: {}", file_name);
    }

    Ok(())
}

pub fn read_file_to_string(file_path: &str) -> Result<String, Error> {
    // 读取文件内容并返回字符串
    let content = fs::read_to_string(file_path)?;
    Ok(content)
}

pub fn write_string_to_file(file_path: &str, content: &str) -> Result<(), Error> {
    // 将字符串内容写入文件
    fs::write(file_path, content)?;
    Ok(())
}