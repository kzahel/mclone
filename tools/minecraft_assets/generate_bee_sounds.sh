#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
output_dir="$repo_root/assets/mclone/sounds/mclone-original"
mkdir -p "$output_dir"

generate_buzz() {
  local output="$1"
  local expression="$2"
  local duration="$3"
  ffmpeg -hide_banner -loglevel error -y \
    -f lavfi -i "aevalsrc=${expression}:s=44100:d=${duration}" \
    -filter_complex "highpass=f=110,lowpass=f=1100,acompressor=threshold=0.16:ratio=4:attack=3:release=60,afade=t=in:st=0:d=0.05,afade=t=out:st=$(awk -v d="$duration" 'BEGIN { print d - 0.08 }'):d=0.08,aformat=channel_layouts=stereo" \
    -c:a vorbis -strict experimental -q:a 5 "$output"
}

generate_buzz "$output_dir/bee_buzz_00.ogg" '0.18*sin(2*PI*(205+18*sin(2*PI*7*t))*t)+0.06*sin(2*PI*410*t)' 0.72
generate_buzz "$output_dir/bee_buzz_01.ogg" '0.17*sin(2*PI*(225+22*sin(2*PI*6*t))*t)+0.05*sin(2*PI*450*t)' 0.66
