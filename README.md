# comic_dl

download comic images from web.

## How

```bash
Usage: comic_dl.exe [OPTIONS] --url <URL>

Options:
  -u, --url <URL>          comic website url
  -e, --element <ELEMENT>  which element that contains comic images [default: .uk-zjimg]
  -a, --attr <ATTR>        image element src attr [default: data-src]
  -f, --file <FILE>        save filepath name [default: ./output]
  -d, --dl-type <DL_TYPE>  download type, "juan" "hua" "fanwai" "current" [default: current] [possible values: juan, hua, fanwai, current, local, upscale]
  -h, --help               Print help
  -V, --version            Print version
```

## Support Site

* antbyw
* mangadex

## Example

```bash
# local image process
cargo run -- -u "C:\Users\hahaz\Downloads\王者天下_单行本" -d "upscale"
cargo run -- -u "C:\Users\hahaz\Downloads\王者天下_单行本" -d "local"

# antbyw
cargo run -- -u "https://www.antbyw.com/plugin.php?id=jameson_manhua&c=index&a=bofang&kuid=143450" -d "juan"
cargo run -- -u "https://www.antbyw.com/plugin.php?id=jameson_manhua&c=index&a=bofang&kuid=143450" -d "hua"
cargo run -- -u "https://www.antbyw.com/plugin.php?id=jameson_manhua&c=index&a=bofang&kuid=143450" -d "fanwai"

# mangadex
cargo run -- -u "https://mangadex.org/title/40bc649f-7b49-4645-859e-6cd94136e722/dragon-ball"
```

## Changelog

* `version 1.0.0` support antbyw and mangadex, antbyw download have .json cache file.