#!/bin/sh
set -u

bin="$1"
identifier="$2"
watch_stamp="$3"
log_path="$4"
stable_size=""
stable_state=""
stable_count=0
attempt=0

binary_state() {
    if [ -f "$bin" ]; then
        /usr/bin/stat -f '%i:%m:%z' "$bin" 2>/dev/null || true
    fi
}

initial_state="$(binary_state)"

while [ "$attempt" -lt 2400 ]; do
    state="$(binary_state)"
    if [ -n "$state" ] && [ "$state" != "$initial_state" ]; then
        size="${state##*:}"
        if [ -n "$size" ] && [ "$state" = "$stable_state" ]; then
            stable_count=$((stable_count + 1))
        else
            stable_size="$size"
            stable_state="$state"
            stable_count=0
        fi

        if [ "$stable_count" -ge 20 ]; then
            /usr/bin/codesign --force --sign - --identifier "$identifier" "$bin" >>"$log_path" 2>&1
            /usr/bin/touch "$watch_stamp"
            exit 0
        fi
    fi

    attempt=$((attempt + 1))
    /bin/sleep 0.05
done

if [ -f "$bin" ]; then
    echo "timed out waiting for relink; signing existing $bin" >>"$log_path"
    /usr/bin/codesign --force --sign - --identifier "$identifier" "$bin" >>"$log_path" 2>&1
else
    echo "timed out waiting to ad-hoc sign $bin" >>"$log_path"
fi
/usr/bin/touch "$watch_stamp"
exit 0
