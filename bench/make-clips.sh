#!/bin/sh
# Generates the two clips the bench plays. They are not committed: a repository
# whose whole point is a 522 KB binary has no business carrying 4 MB of video.
#
# Synthetic footage decodes differently from a real reel, which does not matter
# here — the bench measures what building and tearing down a pipeline costs,
# not what decoding one frame costs. What matters is that the resolution,
# frame rate and profile match what Instagram serves: 1080x1920, 30 fps,
# H.264 High.
set -eu
cd "$(dirname "$0")"

command -v ffmpeg >/dev/null || { echo "ffmpeg is required" >&2; exit 1; }

# The bitrate cap matters as much as the resolution. Left uncapped, synthetic
# footage encodes to roughly twice what Instagram serves, and the bench then
# measures how fast the little Python server can push bytes rather than what
# the engine does with them: videos stall, `playedSec` collapses and the
# numbers look great for the wrong reason. 3 Mbps is what a real reel costs.
common="-f lavfi -i testsrc2=size=1080x1920:rate=30 -t 5 -an
        -c:v libx264 -profile:v high -level 4.0 -pix_fmt yuv420p -g 30
        -b:v 3000k -maxrate 3000k -bufsize 6000k"

# Progressive: what a plain <video src="...mp4"> loads.
# shellcheck disable=SC2086
ffmpeg -v error -y $common clip5.mp4

# Fragmented: what a MediaSource is fed, one append per element.
# shellcheck disable=SC2086
ffmpeg -v error -y $common \
    -movflags frag_keyframe+empty_moov+default_base_moof -f mp4 clip5_frag.mp4

ls -l clip5.mp4 clip5_frag.mp4
