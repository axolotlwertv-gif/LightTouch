//! Live firmware catalogue for newer iPhone and iPad products. Listing a firmware here does not
//! imply that the QEMU engine can prepare or boot the corresponding hardware.
use serde::Deserialize;
use std::time::Duration;

const USER_AGENT: &str = concat!("LightTouch/", env!("CARGO_PKG_VERSION"), " (+https://github.com/blake-lucas/LightTouch)");

#[derive(Debug, Clone, Deserialize)]
pub struct Device {
    pub identifier: String,
    pub name: String,
    #[serde(default)]
    pub platform: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Firmware {
    pub version: String,
    pub buildid: String,
    #[serde(default)]
    pub releasedate: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub filesize: u64,
    #[serde(default)]
    pub sha1sum: String,
    #[serde(default)]
    pub signed: bool,
}

#[derive(Debug, Deserialize)]
struct DeviceFirmwareResponse {
    #[serde(default)]
    firmwares: Vec<Firmware>,
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .user_agent(USER_AGENT)
        .timeout_connect(Some(Duration::from_secs(12)))
        .timeout_recv_response(Some(Duration::from_secs(20)))
        .timeout_recv_body(Some(Duration::from_secs(30)))
        .https_only(true)
        .build()
        .into()
}

fn generation(identifier: &str, prefix: &str) -> Option<u32> {
    identifier.strip_prefix(prefix)?.split(',').next()?.parse().ok()
}

/// iPhone 8 is iPhone10,x. The iPad2,x family starts with iPad 2 and also includes iPad mini 1.
pub fn is_requested_device(device: &Device) -> bool {
    if device.identifier.starts_with("iPhone") {
        return generation(&device.identifier, "iPhone").is_some_and(|n| n >= 10);
    }
    if device.identifier.starts_with("iPad") {
        return generation(&device.identifier, "iPad").is_some_and(|n| n >= 2);
    }
    false
}

/// Returns all IPSW.me products requested for the extended selector.
pub fn devices() -> Result<Vec<Device>, String> {
    let mut response = agent()
        .get("https://api.ipsw.me/v4/devices")
        .header("Accept", "application/json")
        .call()
        .map_err(|error| format!("Could not load IPSW.me devices: {error}"))?;
    let mut devices: Vec<Device> = response
        .body_mut()
        .read_json()
        .map_err(|error| format!("Could not parse IPSW.me devices: {error}"))?;
    devices.retain(is_requested_device);
    devices.sort_by(|a, b| {
        let family_a = if a.identifier.starts_with("iPhone") { 0 } else { 1 };
        let family_b = if b.identifier.starts_with("iPhone") { 0 } else { 1 };
        family_a.cmp(&family_b).then_with(|| a.name.cmp(&b.name)).then_with(|| a.identifier.cmp(&b.identifier))
    });
    Ok(devices)
}

/// Returns IPSW restore releases for one exact Apple ProductType, signed and unsigned.
pub fn firmwares(product_type: &str) -> Result<Vec<Firmware>, String> {
    if !(product_type.starts_with("iPhone") || product_type.starts_with("iPad"))
        || !product_type.bytes().all(|b| b.is_ascii_alphanumeric() || b == b',')
    {
        return Err("Invalid iPhone/iPad product identifier.".into());
    }
    let url = format!("https://api.ipsw.me/v4/device/{product_type}?type=ipsw");
    let mut response = agent()
        .get(&url)
        .header("Accept", "application/json")
        .call()
        .map_err(|error| format!("Could not load IPSW.me firmware for {product_type}: {error}"))?;
    let mut result: DeviceFirmwareResponse = response
        .body_mut()
        .read_json()
        .map_err(|error| format!("Could not parse IPSW.me firmware for {product_type}: {error}"))?;
    result.firmwares.retain(|firmware| firmware.url.starts_with("https://"));
    result.firmwares.sort_by(|a, b| {
        crate::catalog::compare_versions(&b.version, &a.version).then_with(|| b.buildid.cmp(&a.buildid))
    });
    Ok(result.firmwares)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(identifier: &str) -> Device {
        Device { identifier: identifier.into(), name: identifier.into(), platform: String::new() }
    }

    #[test]
    fn includes_iphone_8_and_newer() {
        assert!(!is_requested_device(&device("iPhone9,3")));
        assert!(is_requested_device(&device("iPhone10,1")));
        assert!(is_requested_device(&device("iPhone10,3")));
        assert!(is_requested_device(&device("iPhone17,1")));
    }

    #[test]
    fn includes_ipad_2_and_all_ipad_families() {
        assert!(!is_requested_device(&device("iPad1,1")));
        assert!(is_requested_device(&device("iPad2,1")));
        assert!(is_requested_device(&device("iPad2,5"))); // iPad mini
        assert!(is_requested_device(&device("iPad4,1"))); // iPad Air
        assert!(is_requested_device(&device("iPad6,3"))); // iPad Pro
    }

    #[test]
    fn excludes_ipod() {
        assert!(!is_requested_device(&device("iPod7,1")));
    }
}
