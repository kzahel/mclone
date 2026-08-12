#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
output_dir="$repo_root/assets/mclone/sounds/mclone-original"
mkdir -p "$output_dir"

generate_tone() {
  local output="$1"
  local expression="$2"
  local duration="$3"
  ffmpeg -hide_banner -loglevel error -y \
    -f lavfi -i "aevalsrc=${expression}:s=44100:d=${duration}" \
    -filter_complex "highpass=f=90,lowpass=f=2400,acompressor=threshold=0.22:ratio=3:attack=2:release=45,afade=t=out:st=$(awk -v d="$duration" 'BEGIN { print d - 0.06 }'):d=0.06,aformat=channel_layouts=stereo" \
    -c:a vorbis -strict experimental -q:a 5 "$output"
}

generate_tone "$output_dir/deer_contact_00.ogg" '0.52*sin(2*PI*(245*t-95*t*t))*exp(-7*t)' 0.42
generate_tone "$output_dir/deer_contact_01.ogg" '0.48*sin(2*PI*(220*t-70*t*t))*exp(-6*t)' 0.46
generate_tone "$output_dir/deer_alarm_00.ogg" '0.68*sin(2*PI*(480*t-150*t*t))*exp(-13*t)+0.22*sin(2*PI*960*t)*exp(-18*t)' 0.28
generate_tone "$output_dir/deer_alarm_01.ogg" '0.62*sin(2*PI*(430*t-120*t*t))*exp(-12*t)+0.19*sin(2*PI*860*t)*exp(-17*t)' 0.30
generate_tone "$output_dir/deer_impact_00.ogg" '0.66*sin(2*PI*(105*t))*exp(-28*t)+0.22*sin(2*PI*260*t)*exp(-35*t)' 0.22
generate_tone "$output_dir/deer_impact_01.ogg" '0.62*sin(2*PI*(92*t))*exp(-26*t)+0.20*sin(2*PI*225*t)*exp(-32*t)' 0.24
