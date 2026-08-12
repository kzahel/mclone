#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
output_dir="$repo_root/assets/mclone/sounds/mclone-original"
mkdir -p "$output_dir"

generate_call() {
  local output="$1"
  local base_frequency="$2"
  local fall="$3"
  local second_delay="$4"
  ffmpeg -hide_banner -loglevel error -y \
    -f lavfi -i "aevalsrc=0.74*sin(2*PI*(${base_frequency}*t-${fall}*t*t))*exp(-8*t)*between(t\\,0\\,0.22)+0.58*sin(2*PI*((${base_frequency}-70)*(t-${second_delay})-${fall}*(t-${second_delay})*(t-${second_delay})))*exp(-9*(t-${second_delay}))*between(t\\,${second_delay}\\,${second_delay}+0.20):s=44100:d=0.48" \
    -f lavfi -i "anoisesrc=color=pink:amplitude=0.17:sample_rate=44100:duration=0.48" \
    -filter_complex "[1:a]bandpass=f=900:w=1200[n];[0:a][n]amix=inputs=2:weights='1 0.34',highpass=f=180,lowpass=f=2100,acompressor=threshold=0.22:ratio=3:attack=2:release=35,afade=t=out:st=0.42:d=0.06,aformat=channel_layouts=stereo" \
    -c:a vorbis -strict experimental -q:a 5 "$output"
}

generate_call "$output_dir/mallard_call_00.ogg" 720 540 0.19
generate_call "$output_dir/mallard_call_01.ogg" 655 460 0.17
