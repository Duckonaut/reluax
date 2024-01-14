![logo](assets/logo.svg)

# reluax
`reluax` lets you easily create dynamic websites in a custom dialect of Lua,
inspired by JSX. The service has been written in Rust, and uses `luajit` as
its Lua runtime for performance and ease of integration of other libraries.

## Requirements
- `luajit`

## Installation
This project is available through GitHub releases as a prebuild binary, or
from `crates.io` through `cargo install reluax`, if you have the Rust
toolchain installed. Alternatively, it is packaged as a Nix flake:
if you have Nix installed, you can `nix run github:Duckonaut/reluax`.

## Usage
```
Commands:
  serve  Serve a directory of LuaX files in production mode
  build  Build a directory of LuaX files
  dev    Serve a directory of LuaX files in development mode
  new    Create a new project
  init   Initialize a new project in the current directory
  help   Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Examples:
  reluax serve
  reluax dev -P public/ -C luax/
```

To create an example project, run `reluax new my-first-project`.

## Inspiration
The project was heavily inspired by Ben Visness' blog post,
[I made JSX for Lua (because I hate static sites)](https://bvisness.me/luax/),
which I highly recommend reading.
