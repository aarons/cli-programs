# get-image

Generate images from a text prompt on the command line, using OpenRouter image
models. Images are saved to the current working directory, and an interactive
session makes it fast to tweak the prompt or settings and regenerate.

## Usage

```bash
# Generate one image with the defaults (google/gemini-2.5-flash-image, low quality, 512px)
get-image a watercolor fox reading a newspaper

# Pick a model, higher quality, bigger canvas, four copies
get-image --model openai/gpt-image-1 --quality high --size 2K -n 4 "logo sketch"

# Guide generation with reference images (local files or URLs, repeatable)
get-image --reference photo.jpg "make this scene look like a watercolor painting"
get-image -r front.png -r side.png "product shot of this object on a marble table"

# Template groups: [a|b] expands into one generation per combination.
# This generates four images: red cat, red dog, blue cat, blue dog.
get-image "a [red|blue] [cat|dog] in a meadow"

# Generate once and exit without the interactive session
get-image --once a quick test render

# Browse available image models with pricing, cheapest first
get-image models
```

## Filenames and the generation log

Images are named with the generation date plus the first few words of the
prompt, e.g. `2026-07-29-a-watercolor-fox-reading.png`, so names sort
chronologically and stay recognizable. Repeated generations never overwrite
(`...-2.png`, `...-3.png`), and `--output` replaces the whole stem with a
name of your choosing.

Because the filename only carries a fragment of the prompt, every generation
is also appended to `image-generation-log.jsonl` in the working directory —
one JSON line per API call with the timestamp, full prompt, model, quality,
size, reported cost, and the files it produced:

```json
{"time":"2026-07-29T14:35:02-07:00","prompt":"a watercolor fox reading a newspaper","model":"google/gemini-2.5-flash-image","quality":"low","size":"512","cost":0.0034,"files":["2026-07-29-a-watercolor-fox-reading.png"]}
```

The log is plain-greppable (`grep fox image-generation-log.jsonl`) and
parseable with `jq`, e.g. total spend in a directory:
`jq -s 'map(.cost // 0) | add' image-generation-log.jsonl`.

## Inline display

On terminals that can render images — iTerm2, WezTerm, kitty, and Ghostty —
each generated image is also displayed directly in the terminal (via the
iTerm2 inline-image protocol or the kitty graphics protocol, chosen
automatically). Pass `--no-display` to turn this off. Inside tmux or screen,
which don't pass the escape sequences through, display is skipped
automatically.

## Prompt templates

A prompt may contain `[option|option|...]` groups. Each group multiplies the
prompt into one generation per option, and multiple groups combine:
`"a [red|blue] [cat|dog]"` expands to four prompts. Brackets without a `|`
inside are kept as literal text. A template is limited to 16 combinations as
a guard against accidental large bills (each combination also honors
`--count`). Templates work both on the command line and in the interactive
session, and a total cost is printed after multi-prompt runs.

## Interactive session

After the first image, `get-image` stays open (unless `--once` is passed or
stdin is not a terminal):

```
get-image> ⏎               regenerate the same prompt
get-image> <new text>      replace the prompt and generate (Up edits the last prompt);
                           [a|b] template groups expand here too
get-image> /model <id>     switch model
get-image> /quality high   set quality (low, medium, high, auto)
get-image> /size 1024x768  set size (512, 1K, 2K, 4K, 1024, or WIDTHxHEIGHT)
get-image> /count 4        images per generation (1-10)
get-image> /reference p.jpg add a reference image (path or URL); /reference lists, /reference clear removes all
get-image> /open [n]       open an image in the system viewer
get-image> /list           list images generated this session
get-image> /settings       show current settings
get-image> /save           persist current settings as defaults
get-image> /quit           exit
```

## Reference images

`--reference` (or `-r`) passes images to the model for image-to-image
generation — style transfer, edits, "put this object in that scene", and so
on. Local files are embedded as base64 data URLs (png, jpeg, webp, gif); HTTP(S)
URLs are passed through for the provider to fetch. References stay attached for
the whole interactive session until `/reference clear`.

Not every model accepts reference images, and those that do cap how many. Before
sending, `get-image` checks the model's capabilities via OpenRouter's image
models API and refuses with a clear message if the model takes no references or
fewer than you gave it, so you don't pay for a rejected request. If the
capability catalog is unreachable, the request is sent as-is and the API's
answer is reported.

## Authentication

`get-image` needs an OpenRouter API key, resolved in order:

1. `api_key` under `[providers.openrouter]` in `~/.config/cli-programs/llm.toml`
   (shared with the other workspace tools)
2. The `OPENROUTER_API_KEY` environment variable

`get-image models` uses a public endpoint and works without a key.

## Configuration

Defaults live in `~/.config/cli-programs/get-image.toml`:

```toml
model = "google/gemini-2.5-flash-image"
quality = "low"
size = "512"
count = 1
open_after_save = false
```

Manage them from the command line:

```bash
get-image config show
get-image config set model openai/gpt-image-1
get-image config set quality medium
get-image config path
```

The defaults deliberately favor inexpensive settings (`quality = "low"`,
`size = "512"`); raise them per-run with flags or persistently via
`config set`.

## Cost control

- Generation requests set OpenRouter's `provider.sort = "price"` so routing
  prefers the cheapest provider for the chosen model.
- `get-image models` lists image-capable models sorted by estimated price per
  image, so it's easy to find inexpensive options as the catalog changes. The
  estimates assume full quality; token-billed models cost less at lower
  quality and resolution.
- The actual cost reported by OpenRouter is printed after each generation.
- Quality and size support varies by model. Models that don't support a
  quality knob ignore it, and if a model rejects the size setting, get-image
  retries once without it (and says so).

## Notes

- Generation uses OpenRouter's dedicated Images API (`POST /api/v1/images`).
- Multiple copies (`-n`) are separate concurrent requests, because many
  models — including the default — only produce one image per request.
