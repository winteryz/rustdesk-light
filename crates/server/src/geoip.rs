use rdl_protocol::{now_epoch_ms, ClientLocation};
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};

pub(crate) struct GeoIpLocator {
    reader: Option<maxminddb::Reader<Vec<u8>>>,
    path: Option<PathBuf>,
}

impl GeoIpLocator {
    pub(crate) fn open(path: Option<&Path>) -> Self {
        let Some(path) = path else {
            return Self {
                reader: None,
                path: None,
            };
        };
        match maxminddb::Reader::open_readfile(path) {
            Ok(reader) => Self {
                reader: Some(reader),
                path: Some(path.to_path_buf()),
            },
            Err(error) => {
                eprintln!("geoip disabled: {}: {error}", path.display());
                Self {
                    reader: None,
                    path: Some(path.to_path_buf()),
                }
            }
        }
    }

    pub(crate) fn status_label(&self) -> String {
        match (&self.reader, &self.path) {
            (Some(_), Some(path)) => path.display().to_string(),
            (Some(_), None) => "enabled".to_string(),
            (None, Some(path)) => format!("disabled({})", path.display()),
            (None, None) => "disabled".to_string(),
        }
    }

    pub(crate) fn lookup_peer_addr(&self, peer_addr: &str) -> Option<ClientLocation> {
        let reader = self.reader.as_ref()?;
        let ip = peer_ip(peer_addr)?;
        if !geoip_candidate(ip) {
            return None;
        }
        let result = reader.lookup(ip).ok()?;
        let city = result.decode::<maxminddb::geoip2::City>().ok()??;
        let latitude = city.location.latitude?;
        let longitude = city.location.longitude?;
        let accuracy_meters = city
            .location
            .accuracy_radius
            .map(|km| u32::from(km).saturating_mul(1_000))
            .unwrap_or(0);
        Some(ClientLocation::from_degrees(
            latitude,
            longitude,
            accuracy_meters,
            "ip",
            geoip_label(&city),
            now_epoch_ms(),
        ))
    }
}

fn peer_ip(peer_addr: &str) -> Option<IpAddr> {
    peer_addr
        .parse::<SocketAddr>()
        .map(|addr| addr.ip())
        .ok()
        .or_else(|| peer_addr.parse::<IpAddr>().ok())
}

fn geoip_candidate(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            !(ip.is_loopback()
                || ip.is_private()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                || ip.is_multicast()
                || ip.is_unspecified())
        }
        IpAddr::V6(ip) => {
            !(ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_multicast()
                || ip.is_unique_local()
                || ((ip.segments()[0] & 0xffc0) == 0xfe80))
        }
    }
}

fn geoip_label(city: &maxminddb::geoip2::City<'_>) -> String {
    let mut parts = Vec::new();
    if let Some(name) = preferred_name(&city.city.names) {
        parts.push(name.to_string());
    }
    if let Some(subdivision) = city
        .subdivisions
        .first()
        .and_then(|subdivision| preferred_name(&subdivision.names))
    {
        if !parts.iter().any(|part| part == subdivision) {
            parts.push(subdivision.to_string());
        }
    }
    if let Some(country) = preferred_name(&city.country.names).or(city.country.iso_code) {
        if !parts.iter().any(|part| part == country) {
            parts.push(country.to_string());
        }
    }
    if parts.is_empty() {
        "IP geolocation".to_string()
    } else {
        parts.join(", ")
    }
}

fn preferred_name<'a>(names: &'a maxminddb::geoip2::Names<'a>) -> Option<&'a str> {
    names.simplified_chinese.or(names.english)
}
