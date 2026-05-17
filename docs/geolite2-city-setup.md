# GeoLite2 City Setup

The admin client map can show approximate client locations when the server is
started with a MaxMind GeoLite2 or GeoIP2 City database. The database is
optional. Without it, the server still works normally, but client location fields
stay empty and the map has no location points.

## Download the Database

Use the free MaxMind GeoLite2 City database:

```text
https://www.maxmind.com/en/geolite-free-ip-geolocation-data
```

If this is the first time using the account, complete the GeoLite signup and
accept the GeoLite End User License Agreement in MaxMind. After that, open:

```text
My Account -> GeoIP / GeoLite -> Download files
```

In the `Download Databases` table, use this row:

```text
Database:   GeoLite City
Edition ID: GeoLite2-City
Format:    GeoIP2 Binary (.mmdb) (APIs)
```

Click:

```text
Download GZIP
```

Do not use the CSV row for the rust-desk-light server. The server expects the
binary `.mmdb` database.

Other links in the same row are optional:

- `Download SHA256` is only for checksum verification.
- `Download Locations Diff Report` is a data-change report, not the database.
- `Get Permalinks` is for scripted downloads with a MaxMind license key.

## Fix Expired Download Links

MaxMind's `Download GZIP` links are temporary token links. If the browser shows:

```text
Link is invalid or expired. Refresh the page where the link came from to try again.
```

refresh the `Download files` page and click `Download GZIP` again. Do not reuse
old copied links from a previous page load.

For automated updates, use `Get Permalinks` in the MaxMind UI and store the
license key outside the repository, for example in an environment variable or a
server secret manager.

## Extract the Database

After downloading, extract the archive:

```bash
cd ~/Downloads
tar -xzf GeoLite2-City_*.tar.gz
find GeoLite2-City_* -name GeoLite2-City.mmdb -print
```

The expected file is:

```text
GeoLite2-City_YYYYMMDD/GeoLite2-City.mmdb
```

## Install on the Server

For this repository's startup scripts, keep the downloaded archive in the GeoIP
directory and extract the database to a stable filename:

```bash
mkdir -p third_party/geoip
cp ~/Downloads/GeoLite2-City_*.tar.gz third_party/geoip/
tar -xOzf third_party/geoip/GeoLite2-City_*.tar.gz '*/GeoLite2-City.mmdb' > third_party/geoip/GeoLite2-City.mmdb
```

The shell startup scripts use `third_party/geoip/GeoLite2-City.mmdb` directly.
They do not extract archives at startup. The database path has no date in it.

If the file was downloaded on another machine, copy the archive to the server
repository first:

```bash
scp GeoLite2-City_*.tar.gz root@SERVER_IP:/path/to/rust-desk-light/third_party/geoip/
```

## Start the Server

The release startup script auto-detects and extracts the archive:

```bash
./scripts/start-server-release.sh
```

The dev stack launcher uses the same lookup:

```bash
./scripts/start-dev.sh
```

You can still pass the `.mmdb` file manually:

```bash
./rdl-server-cli --ip 0.0.0.0 --port 5169 --geoip-db /path/to/GeoLite2-City.mmdb
```

Or set the environment variable:

```bash
RDL_GEOIP_DB=/path/to/GeoLite2-City.mmdb ./rdl-server-cli --ip 0.0.0.0 --port 5169
```

The server startup log includes the GeoIP status:

```text
rust-desk-light server listening on 0.0.0.0:5169 ... geoip=/path/to/GeoLite2-City.mmdb
```

If the file cannot be opened, the server prints a warning and continues with
GeoIP disabled.

## Map Behavior

GeoIP is based on the client connection IP address. It is approximate and should
be treated as country, region, or city-level context only.

Locations may be empty or misleading when clients connect from:

- loopback addresses such as `127.0.0.1` or `::1`
- private LAN addresses such as `10.x.x.x`, `172.16.x.x`, or `192.168.x.x`
- VPNs, proxies, NAT gateways, relays, or cloud egress endpoints

For a useful map, clients generally need to connect to the server through their
public network path.

## Common Issues

`Link is invalid or expired`

Refresh the MaxMind `Download files` page and click `Download GZIP` again.

`geoip disabled: ...`

Check that the path points to `GeoLite2-City.mmdb`, not the `.tar.gz` archive,
and that the server process can read the file.

No location appears in the admin map

Confirm the server was started with `--geoip-db` or `RDL_GEOIP_DB`. Also check
whether the client is connecting from a local, private, VPN, or proxy IP.

Map location is not the physical client location

This is expected for IP geolocation. It identifies the network endpoint MaxMind
knows about, not a precise device position.
