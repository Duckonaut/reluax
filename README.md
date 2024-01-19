<p align="center"><img src="assets/logo.svg" /></p>

# Reluax

![Crates.io Version](https://img.shields.io/crates/v/reluax)

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
  reluax serve --port 4000
  reluax dev -P public/ -C luax/
```

To create an example project, run `reluax new my-first-project`.

## Inspiration
The project was heavily inspired by Ben Visness' blog post,
[I made JSX for Lua (because I hate static sites)](https://bvisness.me/luax/),
which I highly recommend reading.

## API
Reluax expects to run a directory of files, with the entry point being `reluax.lua(x)`
This module needs to return a table containing the member function `route`.

This function will be called with the path and optionally method and body of a
request, and can return a variety of responses, by returning two values: the
status code, and the response body.

The response body will usually be a table, and by default will be treated as a HTML
page (see `example/basic/`). It can be optionally wrapped using the functions
`reluax.html` or `reluax.json`, the first of which will make sure the HTML is returned
as is, without a `<!DOCTYPE html>` tag, and the second returning the table as a JSON
object.

With this you can build a rather powerful backend, handling templating, routing, and
anything else through LuaX code.

The `reluax` global table contains several utility functions, described below:
- `reluax.json`: wrap the table to be interpreted as a JSON response,
- `reluax.html_page`: wrap the table to be interpreted as a full HTML page (default behavior),
- `reluax.html`: wrap the table to be interpreted as a HTML excerpt (for e.g. use with
  [htmx](https://htmx.org)),
- `reluax.path_matches`: check if a path string matches the template,
- `reluax.path_extract`: extract named path parameters from the path.
