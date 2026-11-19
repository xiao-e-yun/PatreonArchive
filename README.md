# Patreon Archive

> Check [PostArchiver](https://github.com/xiao-e-yun/PostArchiver) to know more info.

It is importer for patreon to PostArchiver.

```sh
Usage: patreon-archive [OPTIONS] <SESSION> [OUTPUT]

Arguments:
  <SESSION>  Your `session_id` cookie [env: SESSION]
  [OUTPUT]   Which you path want to save [env: OUTPUT=] [default: ./archive]

Options:
      --strategy <STRATEGY>         Archiving strategy (`full` re-archives existing posts) [default: increment]
  -w, --whitelist [<WHITELIST>...]  Whitelist of creator IDs
  -b, --blacklist [<BLACKLIST>...]  Blacklist of creator IDs
      --limit <LIMIT>               Limit API requests per minute [default: 60]
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

