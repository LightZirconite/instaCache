# The video-smoothness bench

CPU averages do not measure stutter, and a run against the real Instagram
measures whichever clips happened to be in the feed — two samples there once
disagreed by 3x and produced the opposite of the truth. This bench exists so
that a claim about smoothness can be checked instead of believed.

It counts, from inside the page, every frame that arrived more than 50 ms
late. That is the thing a person actually perceives.

## Running it

```sh
./bench/make-clips.sh                     # once; writes two clips, not committed
cargo build
python3 bench/serve.py &                  # serves the page, collects reports
./bench/run.sh mine app 50 churn file
```

Every configuration gets **two runs, and they have to agree** before the
number means anything.

The last argument picks which of the two video paths is exercised, and they
behave very differently:

| `src` | what the page does | what Instagram uses it for |
|---|---|---|
| `file` | `<video src="clip5.mp4">` | feed and Reels clips |
| `mse` | one `MediaSource` per element | adaptive streams |

`churn` replaces one of the four videos every 500 ms; `static` leaves them
alone. Both keep exactly four videos alive, so the decoding load is identical
and only pipeline renewal changes.

## Reading the report

```json
{"jank":20,"p99":49,"presented":5099,"playedSec":172,"ttffMed":165,"errors":1}
```

| field | meaning | why it is there |
|---|---|---|
| `jank` | frames later than 50 ms | the headline number |
| `p99`, `p95`, `median` | frame interval, ms | one bad second hides inside a good average |
| `presented` | frames actually shown, from `requestVideoFrameCallback` | engine-independent, unlike `decoded` |
| `playedSec` | media seconds played | catches "smooth because nothing is playing" |
| `ttffMed` | ms from element created to first frame | catches "smooth because every video starts late" |
| `errors` | `<video>` elements that failed | catches a fix that is really a breakage |

The last four exist because each of them, on this project, once turned an
apparent win into a measured loss. `decoded` in particular is not comparable
between engines — it reported half the truth under `playbin3` — which is why
`presented` was added and `decoded` is kept only for continuity.

## Measured on the reference machine

Steam Deck (AMD Van Gogh, `radeonsi`), CachyOS, four 1080x1920 H.264 streams
at 30 fps, one replaced every 500 ms. Two concordant runs each.

| engine / setting | path | jank | p99 | shown | first frame | failed |
|---|---|---|---|---|---|---|
| **instaCache on Qt WebEngine** (ships) | file | **1 to 6** | 33-50 ms | 4176-4777 | **48-64 ms** | 0 |
| WebKitGTK 4.1, as shipped in 1.2.0 | file | 78 | 70 ms | 4720 | 264 ms | 0 |
| WebKitGTK 4.1, GL sink off | file | 20 / 30 | 49 / 52 ms | 5099 / 5128 | 165 / 167 ms | 1 / 0 |
| WebKitGTK 4.1 | mse | 4 / 4 | 44 / 42 ms | 4782 | 243 ms | 0 |
| Chromium 43 (Electron), bare | file | 0 | 20 ms | 4253 | 24 ms | 0 |
| Chromium (qt6-webengine), bare | file | 1 / 0 | 33 ms | 4602 / 5225 | 19 / 20 ms | 0 |

`p99` is 33 ms rather than 20 in the Qt rows because that window presents at
30 fps: 33 ms *is* one frame interval there, not a stall. `jank` and `shown`
are the numbers to read.

Three conclusions worth keeping, all of which contradict what this project
believed before the bench could tell the two video paths apart:

- **MediaSource was never the problem.** WebKit handled the `mse` path at 4
  late frames. The path that stalled is the plain `<video src="…mp4">` one,
  which is what a Reels feed uses. The original diagnosis blamed MSE because
  the original bench only ever exercised the other path.
- **The GL video sink was a large part of what a new pipeline cost.** Turning
  it off was the best WebKit ever managed: 20-30 late frames instead of 78.
- **No setting closed the rest of the gap.** That is what decided the engine
  change; the patch that improved WebKit is kept in the history rather than
  thrown away.

## Is the disk cache actually working?

Timing a warm start against a cold one proves nothing over loopback: 12 MB
arrives in 40 ms either way, and the difference disappears into the noise. What
does prove it is whether the warm start reaches the server at all, which is why
`serve.py` counts requests under `/assets/` and answers `/stats`.

```sh
./bench/make-clips.sh                       # also writes bench/assets/
python3 bench/serve.py &
curl -s http://127.0.0.1:8731/stats/reset
# run the app twice against /load.html with the same profile, then:
curl -s http://127.0.0.1:8731/stats
```

Measured on the reference machine, 12 MB of assets declared cacheable:

| run | requests reaching the server |
|---|---|
| cold, empty profile | 12 |
| warm, same profile, app restarted | **0** |
| warm, with the files deleted from the server | **0**, page still loads |
| after `instacache --clear-cache` | 12 again |

The third row is the one that settles it: the bodies come from disk, not from
the server being fast. The fourth shows `--clear-cache` clears what it says.

`httpCacheMaximumSize` was measured and deliberately left unset. The default
already retains the whole working set, and Instagram's media arrives on signed
one-shot CDN URLs that no cache can reuse, so raising the cap has nothing to
act on. A setting that changes no measurement does not earn its place.

## Things that were measured and rejected

| tried | result |
|---|---|
| `WEBKIT_GST_USE_PLAYBIN3=1` | halves the stalls but **22-28 of 99 videos never play** |
| `WEBKIT_GST_VIDEO_DECODING_LIMIT=4` | no effect |
| `WEBKIT_DISABLE_DMABUF_RENDERER=1` | far worse: 153 late frames, nothing presented |
| `video_decoding: "software"` | worse: 86 late frames |
| `video_decoding: "auto"` | no better than `gpu`: 76 |

## Two traps

**The bench is not a Meta host.** `urls.rs` therefore calls it external and the
app hands it to the system browser, leaving its own window blank while the
system browser quietly produces the numbers. This has happened twice, and both
times the readings looked wonderful. `run.sh` turns that routing off for the
run; a bench driven any other way must do the same. Check the reported `engine`
field before believing anything — it says `chromium`, `webkit` or `gecko`, and
it is the only thing that catches this.

**`pkill -f` matches the shell running it** on this machine. `run.sh` uses
process groups.
