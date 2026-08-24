#!/bin/bash
# One run of the churn page against one engine.
#
#   ./bench/run.sh <label> <engine> [seconds] [churn|static] [file|mse]
#
# Prints the last report the page sent, as JSON. Start ./bench/serve.py first.
#
# Engines: app          the shipped binary itself -- this is the one to measure
#          wk41-mini    vanilla WebKitGTK 4.1, the engine this project used
#                       before Qt WebEngine, kept as the before/after reference
#          wk60-mini    WebKitGTK 6.0 (GTK 4), same upstream version
#          electron     Chromium, if a system electron is installed
#          qtwebengine  Chromium as shipped by qt6-webengine, via qml6
#          firefox      Gecko, in a throwaway profile
#
# Two habits this machine forces, both learned the hard way:
#   * `pkill -f <pattern>` matches this script's own shell. Engines are
#     launched with setsid and killed by process group instead.
#   * never point this at the real Instagram: automated navigation there looks
#     like a bot and risks the account.
set -u
S="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$S/.." && pwd)"
PORT="${BENCH_PORT:-8731}"

if [ "$#" -lt 2 ]; then
    sed -n '2,20p' "$0" >&2
    exit 2
fi

label="$1"; engine="$2"; secs="${3:-45}"; mode="${4:-churn}"; src="${5:-file}"
url="http://127.0.0.1:$PORT/churn.html?label=$label&mode=$mode&src=$src"

# Every run gets its own empty profile, so nothing here can touch a real
# session and no run inherits the previous one's cache.
scratch="$S/.scratch/$label"
rm -rf "$scratch"; mkdir -p "$scratch"
export INSTACACHE_DATA_HOME="$scratch/data" \
       INSTACACHE_CACHE_HOME="$scratch/cache" \
       INSTACACHE_CONFIG_HOME="$scratch/config"

launch() {
    case "$engine" in
    app)
        # 127.0.0.1 is not a Meta host, so the app's link routing would hand
        # the bench to the system browser and leave its own window empty --
        # silently measuring the system browser instead. That has happened
        # twice. Turn the routing off for the run rather than widening
        # is_internal(), and check the `engine` field in the report.
        #
        # The profile is named after the run so a still-dying instance from
        # the previous one cannot swallow this launch through the
        # single-instance socket.
        mkdir -p "$INSTACACHE_CONFIG_HOME/profiles/$label"
        printf '{"open_external_links_in_browser": false%s}\n' "${BENCH_CONFIG:-}" \
            > "$INSTACACHE_CONFIG_HOME/profiles/$label/config.json"
        setsid "$REPO/target/debug/instacache" --profile "$label" "$url" \
            >"$S/.scratch/$label.log" 2>&1 &
        ;;
    wk41-mini)
        setsid /usr/lib/webkit2gtk-4.1/MiniBrowser "$url" \
            >"$S/.scratch/$label.log" 2>&1 &
        ;;
    wk60-mini)
        setsid /usr/lib/webkitgtk-6.0/MiniBrowser "$url" \
            >"$S/.scratch/$label.log" 2>&1 &
        ;;
    electron)
        BENCH_URL="$url" setsid electron "$S/runners/electron" \
            >"$S/.scratch/$label.log" 2>&1 &
        ;;
    qtwebengine)
        setsid qml6 "$S/runners/bench.qml" -- "$url" \
            >"$S/.scratch/$label.log" 2>&1 &
        ;;
    firefox)
        mkdir -p "$scratch/ff"
        MOZ_NO_REMOTE=1 setsid firefox --no-remote --profile "$scratch/ff" "$url" \
            >"$S/.scratch/$label.log" 2>&1 &
        ;;
    *)
        echo "unknown engine: $engine" >&2
        exit 2
        ;;
    esac
    echo $!
}

pid=$(launch)
sleep "$secs"
kill -TERM -"$pid" 2>/dev/null
sleep 2
kill -KILL -"$pid" 2>/dev/null
sleep 1

grep -F "\"label\":\"$label\"" "$S/report.jsonl" 2>/dev/null | tail -1
