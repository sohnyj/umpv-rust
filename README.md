# umpv-rust

A single-instance mpv launcher for Windows, written in Rust. Based on the [umpv](https://github.com/mpv-player/mpv/blob/master/TOOLS/umpv) Python script from the mpv project. Opens files in a running mpv window via named pipe IPC, or launches a new instance if none is running.

## Usage

### 1. Register file associations with mpv (mpv-register helper)

```bat
.\mpv.com --register --video-exts=mkv,mp4 --audio-exts= --image-exts= --archive-exts= --playlist-exts=
```

Specify the extensions you want. Leave a category empty (`=`) to skip it.

### 2. Add umpv to mpv's registered extensions

Only processes extensions that were registered by the mpv-register helper (step 1). 

```bat
.\umpv.exe --register
```

> [!NOTE] 
> To set umpv as the default for each extension, go to Windows Settings > App > Default apps > mpv, and select umpv for the desired extensions.

Without arguments, registers with the default loadfile mode (`replace`). Optionally specify a different mode:

```bat
.\umpv.exe --register --loadfile=append+play
```

### 3. Unregister umpv

```bat
.\umpv.exe --unregister
```

Restores all registered extensions back to mpv defaults.

## Options

The `--loadfile=<value>` option controls how files are added to the mpv playlist.

| Value | Description |
|-------|-------------|
| `replace` | Stop current playback and play the new file (default) |
| `append` | Append to the end of the playlist |
| `append+play` | Append, and force playback to start |
| `insert-next` | Insert after the current item |
| `insert-next+play` | Insert after the current item, and force playback to start |

The following flags (deprecated since mpv 0.42) are also accepted:

| Value | Description |
|-------|-------------|
| `append-play` | Equivalent to `append+play` |
| `insert-next-play` | Equivalent to `insert-next+play` |

| Value | Description |
|-------|-------------|
| `insert-at` | Not supported (requires an index argument) |
| `insert-at+play` | Not supported (requires an index argument) |

URLs (`scheme://...`) are passed through to mpv as-is without path resolution.

See the [mpv documentation](https://mpv.io/manual/master/#command-interface-[%3Coptions%3E]]]) for the full list of options.

## Acknowledgements

`mpv-icon.ico` is property of the [mpv project](https://github.com/mpv-player/mpv).
