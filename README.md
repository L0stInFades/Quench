# 淬 Quench

*A suite of soulful reflections on the nature of data — prioritizing harmony over noise, clarity over complexity.*

In the quiet spaces between files, there is a rhythm that exists without demanding attention. **淬** does not rush. It breathes through your archives with the patience of a 14th-century monastery at dawn — where compression becomes meditation, and extraction, a gentle unfolding.

## What It Is

A tool for those who seek stillness in their workflows. For when you need your data to travel light, but your mind to remain unburdened.

**Harmony through form:** tar.zst, tar.lz4, tar.br, tar.gz, zip, and the pure simplicity of tar.  
**Breath in the process:** Stream without buffering the world.  
**Gentle resilience:** When archives falter, we skip the broken notes and continue the melody.

## The Essence

Three codecs, one intention:
- **zstd** — The balanced breath (levels 1-20)
- **lz4** — Swift as intuition  
- **brotli** — Deep compression, like memory folding into itself

No need to name what you hold. We recognize archives by their first whisper — magic bytes speaking before extensions declare themselves.

## The Practice

### To Unfold
```bash
# Let the format reveal itself
cargo run -p zipx-cli -- extract --input path/to/archive.tar.zst --output ./out

# Or guide it gently
cargo run -p zipx-cli -- extract --input path/to/archive.zip --output ./out --format zip
```

### To Gather
```bash
# The quiet compression — zstd, level 3
cargo run -p zipx-cli -- compress --input ./mydir --output ./archive.tar.zst

# Deep folding — level 20
cargo run -p zipx-cli -- compress --input ./mydir --output ./archive.tar.zst --level 20

# Swift breath — LZ4
cargo run -p zipx-cli -- compress --input ./mydir --output ./archive.tar.lz4 --format tar.lz4
```

### Many as One
```bash
# Unfold many archives
cargo run -p zipx-cli -- batch-extract --inputs archive1.tar.zst archive2.zip archive3.tar.lz4 --output-dir ./extracted

# Gather many sources
cargo run -p zipx-cli -- batch-compress --inputs dir1 dir2 dir3 --output-dir ./compressed --format tar.zst
```

## The Space Within (UI)

A Tauri + Svelte shell — not a command center, but a window. Dark as contemplation, simple as breath.

```bash
cd ui
npm install
npm run tauri:dev    # for the journey
cd ui && npm run tauri build  # for the artifact
```

## Building

```bash
# The complete form
cargo build --release

# The core, alone
cargo build --release -p zipx-core

# The command line voice
cargo build --release -p zipx-cli
```

## Architecture (The Bones)

- **core** — Where codecs breathe and containers hold
- **cli** — Words for the terminal  
- **ui** — A visual quietude (`src-tauri` behind, Svelte before)

Root `Cargo.toml` binds them in shared purpose.

---

*For when you need your data to travel light, but your mind to remain unburdened.*  
*淬 — the moment between heat and cool, where form becomes essential.*
