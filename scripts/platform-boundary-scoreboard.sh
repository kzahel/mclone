#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
revision=${1:-}

source_file() {
  local path=$1
  if [[ -n "$revision" ]]; then
    git -C "$repo_root" show "$revision:$path"
  else
    sed -n '1,$p' "$repo_root/$path"
  fi
}

list_files() {
  local path=$1
  local glob=$2
  if [[ -n "$revision" ]]; then
    git -C "$repo_root" ls-tree -r --name-only "$revision" -- "$path" |
      awk -v pattern="$glob" '$0 ~ pattern'
  else
    find "$repo_root/$path" -type f |
      sed "s#^$repo_root/##" |
      awk -v pattern="$glob" '$0 ~ pattern'
  fi
}

sum_lines() {
  local path=$1
  local glob=$2
  local total=0
  local file
  while IFS= read -r file; do
    [[ -n "$file" ]] || continue
    total=$((total + $(source_file "$file" | wc -l)))
  done < <(list_files "$path" "$glob")
  printf '%s' "$total"
}

count_matches() {
  local path=$1
  local needle=$2
  source_file "$path" | awk -v needle="$needle" '
    BEGIN { count = 0 }
    { rest = $0; while ((index_at = index(rest, needle)) != 0) {
        count++; rest = substr(rest, index_at + length(needle));
      }
    }
    END { print count }
  '
}

count_web_scene_exports() {
  source_file native/apps/mclone-web-client/src/web_scene_host.rs | awk '
    /^impl WebSceneHost \{/ { inside = 1 }
    inside && /#\[wasm_bindgen\(js_name/ { count++ }
    inside && /^}/ { print count + 0; exit }
  '
}

authored_ts_lines=0
authored_ts_modules=0
typescript_gate_lines=0
typescript_gate_modules=0
while IFS= read -r file; do
  [[ -n "$file" ]] || continue
  file_lines=$(source_file "$file" | wc -l)
  typescript_gate_lines=$((typescript_gate_lines + file_lines))
  typescript_gate_modules=$((typescript_gate_modules + 1))
  if [[ "$file" != *.d.ts ]]; then
    authored_ts_lines=$((authored_ts_lines + file_lines))
    authored_ts_modules=$((authored_ts_modules + 1))
  fi
done < <(list_files native/apps/mclone-web-client/www '\.ts$')
web_rust_lines=$(sum_lines native/apps/mclone-web-client/src '\.rs$')
scene_rust_lines=$(sum_lines native/crates/mclone-scene/src '\.rs$')
app_runtime_rust_lines=$(sum_lines native/crates/mclone-app-runtime/src '\.rs$')
web_scene_exports=$(count_web_scene_exports)

async_mutable_wasm_exports=0
for needle in \
  'pub async fn start_pending_session(' \
  'pub async fn shutdown_async(' \
  'pub async fn complete_asset_pack_selection(' \
  'pub async fn start(&mut self)'; do
  async_mutable_wasm_exports=$((
    async_mutable_wasm_exports +
    $(count_matches native/apps/mclone-web-client/src/web_scene_host.rs "$needle")
  ))
done

scene_wasm_forks=0
while IFS= read -r file; do
  [[ -n "$file" ]] || continue
  scene_wasm_forks=$((
    scene_wasm_forks +
    $(count_matches "$file" '#[cfg(not(target_arch = "wasm32"))]')
  ))
done < <(list_files native/crates/mclone-scene/src '\.rs$')

app_runtime_wasm_forks=0
while IFS= read -r file; do
  [[ -n "$file" ]] || continue
  app_runtime_wasm_forks=$((
    app_runtime_wasm_forks +
    $(count_matches "$file" '#[cfg(not(target_arch = "wasm32"))]')
  ))
done < <(list_files native/crates/mclone-app-runtime/src '\.rs$')

printf 'platform-boundary scoreboard\n'
printf 'source\t%s\n' "${revision:-worktree}"
printf 'authored_web_typescript_lines\t%s\n' "$authored_ts_lines"
printf 'authored_web_typescript_modules\t%s\n' "$authored_ts_modules"
printf 'typescript_gate_lines\t%s\n' "$typescript_gate_lines"
printf 'typescript_gate_modules\t%s\n' "$typescript_gate_modules"
printf 'web_only_rust_lines\t%s\n' "$web_rust_lines"
printf 'combined_web_boundary_lines\t%s\n' "$((authored_ts_lines + web_rust_lines))"
printf 'shared_scene_rust_lines\t%s\n' "$scene_rust_lines"
printf 'shared_app_runtime_rust_lines\t%s\n' "$app_runtime_rust_lines"
printf 'web_scene_host_exports\t%s\n' "$web_scene_exports"
printf 'async_mutable_wasm_exports\t%s\n' "$async_mutable_wasm_exports"
printf 'scene_session_non_wasm_cfg_forks\t%s\n' "$scene_wasm_forks"
printf 'app_runtime_non_wasm_cfg_forks\t%s\n' "$app_runtime_wasm_forks"
