# Patreon Archive

> Check [PostArchiver](https://github.com/xiao-e-yun/PostArchiver) to know more info.

It is importer for patreon to PostArchiver.

```sh
Usage: patreon-archive [OPTIONS] <SESSION>

Arguments:
  <SESSION>
          Your `session_id` cookie
          
          [env: SESSION]

Options:
  -o, --output <OUTPUT>
          Archive directory to save into
          
          [env: OUTPUT=]
          [default: ./archive]

      --strategy <STRATEGY>
          Archiving strategy (`full` re-archives existing posts)

          Possible values:
          - increment: Only fetch posts newer than the last run, and skip posts already in the archive
          - full:      Fetch everything again and re-import posts already in the archive
          
          [default: increment]

  -w, --whitelist [<WHITELIST>...]
          Only archive these creators (the `name` in `patreon.com/name`)

  -b, --blacklist [<BLACKLIST>...]
          Skip these creators (the `name` in `patreon.com/name`)

      --limit <LIMIT>
          Limit API requests per minute
          
          [default: 60]

      --skip-free
          Skip free post

  -v, --verbose...
          Increase logging verbosity

  -q, --quiet...
          Decrease logging verbosity

  -h, --help
          Print help (see a summary with '-h')
```

## Build

How to build & run code
```sh
cargo run
```

