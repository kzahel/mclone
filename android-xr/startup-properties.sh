#!/usr/bin/env bash

STARTUP_ARGV_INTENT_EXTRA="mclone.startup.argv"
VIEW_POSE_PROPERTY="debug.mclone.xr_view_pose"

MCLONE_XR_STARTUP_PROPERTIES=(
    "$VIEW_POSE_PROPERTY"
)

mclone_xr_startup_property_default() {
    case "$1" in
        "$VIEW_POSE_PROPERTY")
            printf '0'
            ;;
        *)
            printf '0'
            ;;
    esac
}

mclone_xr_set_startup_property() {
    local serial="$1"
    local name="$2"
    local value="$3"
    "$ADB" -s "$serial" shell setprop "$name" "$value"
}

mclone_xr_clear_startup_property() {
    local serial="$1"
    local name="$2"
    mclone_xr_set_startup_property "$serial" "$name" "$(mclone_xr_startup_property_default "$name")"
}

mclone_xr_clear_all_startup_properties() {
    local serial="$1"
    local name
    for name in "${MCLONE_XR_STARTUP_PROPERTIES[@]}"; do
        mclone_xr_clear_startup_property "$serial" "$name"
    done
}

mclone_xr_json_escape() {
    local value="$1"
    value="${value//\\/\\\\}"
    value="${value//\"/\\\"}"
    value="${value//$'\n'/\\n}"
    value="${value//$'\r'/\\r}"
    value="${value//$'\t'/\\t}"
    printf '%s' "$value"
}

mclone_xr_startup_argv_json() {
    local first=1
    local token

    printf '['
    for token in "$@"; do
        if [[ "$first" == "1" ]]; then
            first=0
        else
            printf ','
        fi
        printf '"%s"' "$(mclone_xr_json_escape "$token")"
    done
    printf ']'
}

mclone_xr_shell_quote() {
    local value="$1"
    value="${value//\'/\'\\\'\'}"
    printf "'%s'" "$value"
}
