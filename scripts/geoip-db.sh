#!/usr/bin/env sh

rdl_find_geoip_db() {
  root_dir="$1"
  source_dir="${RDL_GEOIP_SOURCE_DIR:-$root_dir/third_party/geoip}"

  if [ -n "${RDL_GEOIP_DB:-}" ]; then
    if [ -f "$RDL_GEOIP_DB" ]; then
      printf '%s\n' "$RDL_GEOIP_DB"
      return 0
    fi
    echo "RDL_GEOIP_DB is set but does not exist: $RDL_GEOIP_DB" >&2
    return 1
  fi

  db_path="$source_dir/GeoLite2-City.mmdb"
  if [ -f "$db_path" ]; then
    printf '%s\n' "$db_path"
    return 0
  fi

  return 1
}
