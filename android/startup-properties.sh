#!/usr/bin/env bash

STARTUP_ARGV_INTENT_EXTRA="mclone.startup.argv"
# Legacy fallback only. New launch-scoped remote selection should use
# --remote-addr in STARTUP_ARGV_INTENT_EXTRA.
REMOTE_ADDR_PROPERTY="debug.mclone.remote_addr"
REMOTE_ADDR_NONE_SENTINEL="__mclone_none__"

MCLONE_ANDROID_STARTUP_PROPERTIES=(
    "$REMOTE_ADDR_PROPERTY"
)

mclone_android_startup_property_default() {
    case "$1" in
        "$REMOTE_ADDR_PROPERTY")
            printf '%s' "$REMOTE_ADDR_NONE_SENTINEL"
            ;;
        *)
            printf ''
            ;;
    esac
}

mclone_android_set_startup_property() {
    local serial="$1"
    local name="$2"
    local value="$3"
    "$ADB" -s "$serial" shell setprop "$name" "$value"
}

mclone_android_clear_startup_property() {
    local serial="$1"
    local name="$2"
    mclone_android_set_startup_property "$serial" "$name" "$(mclone_android_startup_property_default "$name")"
}

mclone_android_clear_all_startup_properties() {
    local serial="$1"
    local name
    for name in "${MCLONE_ANDROID_STARTUP_PROPERTIES[@]}"; do
        mclone_android_clear_startup_property "$serial" "$name"
    done
}

mclone_android_json_escape() {
    local value="$1"
    value="${value//\\/\\\\}"
    value="${value//\"/\\\"}"
    value="${value//$'\n'/\\n}"
    value="${value//$'\r'/\\r}"
    value="${value//$'\t'/\\t}"
    printf '%s' "$value"
}

mclone_android_startup_argv_json() {
    local first=1
    local token

    printf '['
    for token in "$@"; do
        if [[ "$first" == "1" ]]; then
            first=0
        else
            printf ','
        fi
        printf '"%s"' "$(mclone_android_json_escape "$token")"
    done
    printf ']'
}

mclone_android_shell_quote() {
    local value="$1"
    value="${value//\'/\'\\\'\'}"
    printf "'%s'" "$value"
}
