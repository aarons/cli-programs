# Tauri desktop UI for get-image

Turn `get-image` from a terminal-only tool into a tool with two frontends: the
existing CLI, and a standalone macOS desktop app built with Tauri. The app
browses the images in a directory alongside the list of available models, and
lets the user delete or regenerate images with single keystrokes. To make that
possible without duplicating logic, the non-terminal parts of `get-image`
(OpenRouter client, config, generation log, file naming, prompt templates) are
extracted into a `get-image-core` library crate that both frontends depend on.

## Context

### Why

`get-image` generates images via OpenRouter image models and already has an
interactive readline loop for tweak-and-regenerate. Browsing the results,
though, means squinting at inline terminal images or opening Finder. The
owner wants a fast, keyboard-driven viewer: models on the left, the current
image in the center, details below, and `↑↓`/`←→`/`d`/`r` to drive it.

A native macOS look is explicitly *not* required. The bar is "fast, snappy,
responsive". Tauri was chosen because it keeps the whole backend in Rust
(reusing the existing async client unchanged), renders the UI in the system
WebView (WKWebView — small bundle, instant launch, no bundled Chromium), and
produces a real `.app`.

### Desired behavior

A window with three regions:

- **Left pane — model list.** Every image-capable OpenRouter model, cheapest
  first, showing id, estimated $/image, and the parameters it accepts
  (quality yes/no, supported resolution tiers). One model is selected; the
  selection is the model used for the next generation.
- **Center pane — image viewer.** One image at a time from the working
  directory, scaled to fit. A small indicator like `3 / 12` shows position.
- **Bottom pane — details.** The selected model's description (OpenRouter
  supplies one), and the shown image's generation record from
  `image-generation-log.jsonl`: full prompt, model, quality, size, cost, time.

Keys:

| Key   | Action                                                                 |
|-------|------------------------------------------------------------------------|
| `↑↓`  | Move the model selection                                               |
| `←→`  | Show the previous / next image                                         |
| `d`   | Delete the shown image (confirm first; prefer Trash over hard delete)  |
| `r`   | Regenerate: resubmit the shown image's *original* prompt and settings  |

`r` never overwrites: the existing file stays, a new file is saved with the
usual unique-name logic, and a new log line is appended. While a generation
runs the UI must stay responsive and show that it's working; when it
finishes the new image becomes the shown image.

Out of scope for this plan (natural follow-ups): typing new prompts inside
the app, editing quality/size/count in the app, multi-directory browsing,
code-signing/notarization for distribution. Design so these can be added,
but don't build them.

### Current repository layout

This is a Cargo workspace of independent CLI programs (see root `CLAUDE.md`).
`get-image/` is one member. Relevant facts for this work:

- `update-cli-programs` installs *every* workspace member as a binary into
  `~/.local/bin`, except names in its `EXCLUDED_PACKAGES` constant
  (`update-cli-programs/src/main.rs:11`). A library crate and a Tauri app
  must be added to that list or the installer will try to copy binaries that
  don't exist / don't belong there.
- `changelog-validator` (`cargo test -p changelog-validator`) validates the
  `CHANGELOG.md` of every workspace member. New crates need a conforming
  `CHANGELOG.md` and a `README.md`; copy `gc/CHANGELOG.md` as the template
  and read `changelog-validator/README.md` for the rules.
- Edition is Rust 2024 (`if let ... && let ...` chains are used).
- Naming rules in the user-level `CLAUDE.md` apply: `snake_case`, no
  abbreviations, qualifiers appended (`directory_images`, not `img_dir`).
- Tooling present on this machine: `cargo`, `node`, `npm`. **Not** present:
  `cargo-tauri` (`cargo install tauri-cli --version '^2'`). Node is not
  required if the frontend is plain HTML/CSS/JS (recommended below).

## Implementation Notes

### Current `get-image` source map (`get-image/src/`)

| File                  | Terminal-specific? | Role                                                                                                   |
|-----------------------|--------------------|--------------------------------------------------------------------------------------------------------|
| `openrouter.rs`       | No (mostly)        | `ImageClient` (async, reqwest+tokio): `generate()`, `list_image_models()`, capability catalog, pricing |
| `config.rs`           | No                 | `Config` at `~/.config/cli-programs/get-image.toml`; `parse_quality/size/count` validators             |
| `generation_log.rs`   | No                 | `GenerationRecord` + `append_record()` → `image-generation-log.jsonl` beside the images                |
| `output.rs`           | No                 | base64 decode, filename stem/slug, `unique_image_path`, `save_image`, `open_in_viewer`                 |
| `template.rs`         | No                 | `[a|b]` prompt expansion                                                                               |
| `session.rs`          | **Yes**            | `Session` struct, `generate()` (prints as it goes), readline loop, `/commands`                          |
| `terminal_display.rs` | **Yes**            | iTerm2 / kitty inline image escape sequences                                                           |
| `main.rs`             | **Yes**            | clap args, `models` and `config` subcommands, API-key resolution                                       |

Key types in `openrouter.rs`: `GenerationSettings { model, quality, size,
count }`, `GenerationResult { images: Vec<GeneratedImage>, cost: Option<f64> }`,
`ImageModel { id, name, price_per_image }`, `ModelCapabilities
{ supports_quality, resolution_tiers }`. `ImageClient` is `Clone` and shares
a lazily-fetched capability cache across clones.

Two things in the "not terminal-specific" files still talk to the terminal
and must change for a GUI host:

1. `ImageClient::generate` prints tuning notes with `eprintln!` and tracks
   which models' notes it already printed (`notes_printed`). The GUI can't
   see stderr. Return the notes instead (e.g. `GenerationResult.notes:
   Vec<String>`) and let each frontend decide how to show them. Debug-mode
   request/response dumps may keep using `eprintln!` (or move to the `log`
   crate); they're developer-only.
2. `Session::generate` / `generate_for_prompt` interleave `println!` with
   the work (progress line, saved paths, cost). Split the work from the
   reporting: a core function that performs one generation and returns what
   happened (saved paths, cost, notes), and the CLI printing from that
   return value.

API-key resolution (`main.rs::resolve_api_key`) reads the shared
`llm-client` config (`~/.config/cli-programs/llm.toml`,
`[providers.openrouter].api_key`) then `OPENROUTER_API_KEY`. The app needs
the same logic, so it moves to core too.

### OpenRouter details the app needs

- `GET https://openrouter.ai/api/v1/models?output_modalities=image&sort=pricing-low-to-high`
  — public, no key. Each entry has `id`, `name`, `description` (free text,
  often a paragraph), `architecture.output_modalities`, `pricing`. The
  existing `ModelEntry` deserializer ignores `description`; add it and carry
  it through `ImageModel` for the bottom pane.
- Capabilities (which tuning parameters a model accepts) come from a second
  catalog already fetched lazily by `ImageClient::model_capabilities`. For
  the left pane's "parameters" column you'll want the whole catalog at once
  rather than one model at a time — expose a method that returns
  `HashMap<model_id, ModelCapabilities>` (the cache already holds exactly
  that).
- Generation cost: BYOK accounts report `usage.cost = 0` with the real
  charge in `cost_details.upstream_inference_cost`; `UsageResponse::
  effective_cost` already handles this. Don't touch it.

### Tauri specifics

Use **Tauri 2**. Recommended shape:

- Frontend: plain `index.html` + one CSS file + one JS module in
  `get-image-app/ui/`, with `build.frontendDist = "../ui"` and no
  `beforeDevCommand`/`beforeBuildCommand`. This avoids a Node toolchain
  entirely; `cargo tauri dev` serves the folder directly. A framework is
  unnecessary for three panes and five keybindings.
- Backend ↔ frontend: `#[tauri::command]` functions invoked from JS via
  `window.__TAURI__.core.invoke` (enable `app.withGlobalTauri = true` in
  `tauri.conf.json` so no bundler is needed). Async commands are fine —
  Tauri's runtime is tokio, so `ImageClient` works as-is. Put `ImageClient`
  and the current settings in Tauri managed state (`app.manage(...)`).
- Long-running generation: the `generate`/`regenerate` command should spawn
  the work with `tauri::async_runtime::spawn` and return immediately;
  completion is reported with `app.emit("generation-finished", payload)` /
  `"generation-failed"`. The frontend listens with `window.__TAURI__.event.
  listen`. Don't block the command until the image arrives — the UI must
  keep taking keystrokes.
- Showing images: use the asset protocol. In `tauri.conf.json` enable
  `app.security.assetProtocol.enable = true` with a `scope` covering the
  images directory, and in JS build the `<img src>` with
  `window.__TAURI__.core.convertFileSrc(path)`. Because the scope must be
  known at build time, scope `$HOME/**` (or the configured images directory
  pattern) rather than trying to compute it at runtime. Alternative if the
  scope is awkward: a command that reads the file and returns base64, used as
  a `data:` URL — simpler, slightly slower for large PNGs.
- Plugins needed: `tauri-plugin-dialog` (folder picker + delete
  confirmation). Optionally `trash` crate (pure Rust, moves to macOS Trash)
  for `d` instead of `std::fs::remove_file`.
- CSP: set `app.security.csp` to allow `img-src 'self' asset: http://asset.localhost data:` or image loads will be blocked silently.
- Icons: `cargo tauri icon path/to/icon.png` generates the icon set the
  bundler requires; without it `cargo tauri build` fails.

### Working directory

The CLI writes to the current directory. A double-clicked `.app` has no
meaningful cwd, so the app needs an explicit images directory:

- Add `images_directory: Option<PathBuf>` to `Config` (and to `config set`
  so it's scriptable). When unset, the app prompts with the folder picker on
  launch and offers to save the choice.
- Accept a directory as a CLI argument too (`get-image-app ~/Pictures/gen`)
  so `cargo tauri dev -- -- <dir>` and shell launches work.

### Image catalog (what `←→` walks)

The list of images is the set of image files (`png`, `jpg`, `jpeg`, `webp`)
in the images directory, sorted by filename (names start with the date, so
this is chronological), each joined to its `GenerationRecord` by matching
`record.files` entries to the filename. Files with no record (copied in by
hand, or logged before this feature) are still shown; their details pane
just says "no generation record" and `r` is disabled for them. The log is
append-only and never rewritten on delete; the on-disk file set is the
source of truth for what's shown.

Recommend putting this join in core as something like
`image_catalog::scan(directory) -> Vec<CatalogImage { path, record: Option<GenerationRecord> }>`
so it is unit-testable without Tauri and reusable by a future CLI `/list`
improvement. `GenerationRecord` currently derives only `Serialize`; add
`Deserialize` (and `Clone`).

## Suggested Approach

Work in this order; each step leaves the CLI building and its tests green.

### 1. Extract `get-image-core`

- New workspace member `get-image-core/` (library). Move `openrouter.rs`,
  `config.rs`, `generation_log.rs`, `output.rs`, `template.rs` and
  `resolve_api_key` into it, keeping their tests. Add `image_catalog.rs`.
- Make `ImageClient::generate` return notes instead of printing them; drop
  `notes_printed` from the client (the CLI can dedupe by model itself if it
  wants the quiet-repeat behavior).
- Add `description` to `ImageModel`; add a method exposing the full
  capability map.
- `get-image` depends on `get-image-core = { path = "../get-image-core" }`;
  `main.rs`/`session.rs`/`terminal_display.rs` stay in the CLI crate.
  `Session::generate_for_prompt` is refactored so the generate-and-save step
  (decode, pick stem, save files, append log record) lives in core as a
  single function returning `{ saved_paths, cost, notes }`, with the CLI
  doing the printing / inline display / `--open` around it. The app calls
  the same core function.
- Add `get-image-core` to `update-cli-programs`' `EXCLUDED_PACKAGES`.
- README + CHANGELOG for the new crate; `cargo test -p changelog-validator`.

### 2. Scaffold `get-image-app`

- `cargo install tauri-cli --version '^2'`, then create `get-image-app/`
  with `src-tauri`-style layout flattened to the workspace convention
  (`get-image-app/Cargo.toml`, `src/main.rs`, `tauri.conf.json`, `ui/`,
  `icons/`). Add to workspace members and to `EXCLUDED_PACKAGES`.
- Prove the pipeline first: one command `list_images(directory)` and an
  `<img>` showing the first file via the asset protocol. This is the only
  step with real unknowns (asset scope / CSP); get it working before
  building the rest.

### 3. Commands and state

Managed state: `ImageClient`, `Config`, selected model id, images directory.
Commands (all return `Result<T, String>` — Tauri serializes the `Err` for JS):

- `list_models() -> Vec<ModelSummary { id, name, description, price_per_image, supports_quality, resolution_tiers }>`
- `list_images() -> Vec<CatalogImageSummary { path, file_name, record: Option<GenerationRecord> }>`
- `select_model(id)`
- `regenerate(file_name)` — looks up the record, builds
  `GenerationSettings { model: record.model, quality, size, count: 1 }`,
  spawns the generation, emits `generation-finished { saved_paths }` or
  `generation-failed { message }`. Decide and document whether `r` uses the
  record's model or the currently selected model; the request says
  "original prompt", and using the record's *settings* too is the most
  predictable reading — the selected model then only matters for future
  "generate new prompt" work. (If you choose the selected model instead,
  say so in the details pane.)
- `delete_image(file_name)` — Trash (or remove), return the refreshed list.
- `choose_images_directory()` — folder picker, persist to config.

### 4. Frontend

- CSS grid: `grid-template-columns: 20rem 1fr; grid-template-rows: 1fr 10rem`.
  Model list is a scrollable `<ul>` with the selected row highlighted and
  kept in view (`scrollIntoView`). Center is a single `<img>` with
  `object-fit: contain`. Bottom pane is two columns: model description,
  image record.
- One `keydown` handler on `window` mapping the five keys; ignore repeats
  while a generation is in flight for `r`, and show a spinner/"Generating
  with <model>…" in the center overlay until the event arrives.
- On `generation-finished`, refresh the image list and jump to the new file.
- On `generation-failed`, show the message in the bottom pane (and any notes
  from a successful generation, e.g. "model has no size setting").
- No state management library; a single `state` object and a `render()`
  function is enough.

### 5. Build

`cargo tauri build` in `get-image-app/` produces
`target/release/bundle/macos/get-image.app`. Document how to copy it to
`/Applications` (or symlink) in the app README. Signing/notarizing is
deferred.

## Testing

Avoid introducing boilerplate tests; we do not want excessive pointless tests as these do not serve anyone.
It's extremely important that the tests are meaningful, clear, and validate core issues and behavior.
It's important to figure out tests that validate our business case, and that ensure healthy core architecture.
They can and should help engineers understand the intention behind the code.

Existing tests in the moved modules must keep passing under `get-image-core`
unchanged. New tests worth writing, all in core (the Tauri commands are thin
glue and should not be unit-tested):

- `image_catalog::scan`: a temp directory with three image files and a log
  covering two of them yields three entries in filename order, the right two
  carrying records, the third `None`; a non-image file is ignored; a log
  line that fails to parse does not abort the scan.
- Regenerate settings are derived from a `GenerationRecord` (prompt, model,
  quality, size; `count` forced to 1) — a small pure function worth a test
  because it encodes the "original settings, not current" decision.
- `ImageClient::generate` returns tuning notes in the result rather than
  printing (the existing `plan_tuning` tests already cover the note text;
  just ensure the plumbing test exists if you add a result-building helper).
- Any `Config` change (`images_directory`) gets covered by extending the
  existing config round-trip test, not a new one.

## Validation

- [ ] `cargo build --release` and `cargo test` pass for the whole workspace,
      including `cargo test -p changelog-validator`.
- [ ] `get-image "a fox" --once` still generates, saves, logs, and displays
      inline exactly as before; the interactive session still works.
- [ ] `cargo run -p update-cli-programs --release` installs the CLI tools and
      does not attempt to install `get-image-core` or `get-image-app`.
- [ ] `cargo tauri dev` (from `get-image-app/`) opens a window showing the
      model list, the first image in the chosen directory, and its record.
- [ ] `↑`/`↓` move the model selection and update the description pane;
      `←`/`→` wrap or stop at the ends without errors; the position indicator
      updates.
- [ ] `r` on an image with a record produces a new file beside the original
      (original untouched), a new line in `image-generation-log.jsonl`, and
      the UI jumps to the new image; the UI accepted keystrokes during the
      wait.
- [ ] `r` on an image with no record is a no-op with a visible explanation.
- [ ] `d` asks for confirmation, removes the file (Trash preferred), and the
      viewer moves to a neighbor; the log file is unchanged.
- [ ] Launching with no `images_directory` configured prompts for a folder
      and remembers it.
- [ ] `cargo tauri build` produces a `.app` that launches from Finder and
      behaves the same as `cargo tauri dev`.
- [ ] Resizing the window keeps the image fitted and the panes intact.
- [ ] Stakeholder review of feel: keystroke-to-redraw is immediate; model
      list scroll is smooth; nothing in the UI blocks during generation.

## Documentation

- `get-image/README.md` — note that the CLI now builds on `get-image-core`,
  and link to the app.
- New `get-image-core/README.md` and `CHANGELOG.md` — what the crate offers
  and who consumes it.
- New `get-image-app/README.md` and `CHANGELOG.md` — prerequisites
  (`tauri-cli`), `cargo tauri dev` / `build`, how to install the `.app`, the
  images-directory config, and the key table above.
- `get-image/CHANGELOG.md` — version bump; the notes-returned-not-printed
  change is user-visible only if CLI output changes, so mention it if so.
- Root `CLAUDE.md` — add `get-image-core` to the development tools list and
  `get-image-app` to installable programs (with a note that it is installed
  as an `.app`, not via `update-cli-programs`); root `README.md` if it lists
  programs.
- `update-cli-programs` — `EXCLUDED_PACKAGES` comment explaining why the two
  new crates are excluded; bump its CHANGELOG if the README mentions the
  list.
