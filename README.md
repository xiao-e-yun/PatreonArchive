# Fanbox Archive

```sh
Usage: fanbox-archive [OPTIONS] <SESSION>

Arguments:
  <SESSION>  Your `FANBOXSESSID` cookie [env: FANBOXSESSID=]

Options:
  -o, --output <OUTPUT>  Which you path want to save [default: ./fanbox]
  -s, --save <SAVE>      Which you type want to save [default: supporting] [possible values: all, following, supporting]
  -c, --cache <CACHE>    Cache directory [default: "."]
  -n, --no-cache         Overwrite existing files
  -l, --limit <LIMIT>    Limit download concurrency [default: 5]
  -v, --verbose...       Increase logging verbosity
  -q, --quiet...         Decrease logging verbosity
  -h, --help             Print help
```

Export (Type ref [PostArchiver](https://github.com/xiao-e-yun/PostArchiver))
```
|- authors.json
|- [AUTHOR]
   |- author.json
   |- [AUTHOR]
      |- post.json
      |- [FILES]
```

## Build

How to build & run code
```sh
cargo run
```