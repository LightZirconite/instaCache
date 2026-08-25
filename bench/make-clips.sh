#!/bin/sh
# Generates what the bench plays and loads. None of it is committed: a
# repository whose whole point is a small binary has no business carrying
# megabytes of video and filler.
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

# Filler for load.html: incompressible, so nothing is won by compression, and
# sized like the bundle a web app pulls on every start.
mkdir -p assets
i=0
while [ "$i" -lt 12 ]; do
    [ -f "assets/chunk$i.bin" ] || head -c 1048576 /dev/urandom > "assets/chunk$i.bin"
    i=$((i + 1))
done

ls -l clip5.mp4 clip5_frag.mp4
du -sh assets
