# Patreon Archive

> Check [PostArchiver](https://github.com/xiao-e-yun/PostArchiver) to know more info.

It is importer for patreon to PostArchiver.

```sh
Usage: patreon-archive [OPTIONS] <SESSION> [OUTPUT]

Arguments:
  <SESSION>  Your `session_id` cookie [env: SESSION=]
  [OUTPUT]   Which you path want to save [env: OUTPUT=] [default: ./archive]

Options:
  -s, --save <SAVE>                 Which you type want to save [env: SAVE=] [default: supporting] [possible values: all, following, supporting]
  -f, --force                       Force download
  -o, --overwrite                   Overwrite existing files
  -w, --whitelist [<WHITELIST>...]  Whitelist of creator IDs
  -b, --blacklist [<BLACKLIST>...]  Blacklist of creator IDs
      --limit <LIMIT>               Limit download concurrency [default: 5]
      --skip-free                   Skip free post
  -v, --verbose...                  Increase logging verbosity
  -q, --quiet...                    Decrease logging verbosity
  -h, --help                        Print help
```

## Build

How to build & run code
```sh
cargo run
```

