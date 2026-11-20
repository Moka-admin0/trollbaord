use crate::audio::DeviceList;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DeviceConfig {
    #[serde(alias = "capture")]
    pub record_from: String,
    #[serde(alias = "send")]
    pub send_to: String,
    pub forward_mic: bool,
    pub mic: String,
    #[serde(alias = "monitor")]
    pub hear_clips: bool,
}

#[derive(PartialEq, Eq)]
pub enum Route {
    GoXlr,
    VbCable,
    Other,
}

impl DeviceConfig {
    pub fn detect(devices: &DeviceList) -> Option<Self> {
        Self::detect_goxlr(devices).or_else(|| Self::detect_vb_cable(devices))
    }

    fn detect_goxlr(devices: &DeviceList) -> Option<Self> {
        let send_to = find_output(devices, &["sample", "goxlr"])?;
        let record_from = find_output(devices, &["chat", "goxlr"])?;
        Some(Self { record_from, send_to, ..Self::default() })
    }

    fn detect_vb_cable(devices: &DeviceList) -> Option<Self> {
        let send_to = find_output(devices, &["cable input"])?;
        let record_from = devices
            .default_output
            .clone()
            .filter(|name| *name != send_to)
            .or_else(|| devices.outputs.iter().find(|name| **name != send_to).cloned())?;
        let mic = devices
            .default_input
            .clone()
            .filter(|name| !is_virtual_cable(name))
            .or_else(|| devices.inputs.iter().find(|name| !is_virtual_cable(name)).cloned())
            .unwrap_or_default();
        Some(Self { record_from, send_to, forward_mic: !mic.is_empty(), mic, hear_clips: true })
    }

    pub fn route(&self) -> Route {
        let send_to = self.send_to.to_lowercase();
        if send_to.contains("goxlr") {
            Route::GoXlr
        } else if send_to.contains("cable") {
            Route::VbCable
        } else {
            Route::Other
        }
    }

    pub fn forwards_mic(&self) -> bool {
        self.forward_mic && !self.mic.is_empty()
    }

    pub fn validate(&self) -> Result<()> {
        if self.record_from.is_empty() {
            bail!("Pick the device you hear Discord on (\"Record from\").");
        }
        if self.send_to.is_empty() {
            bail!("Pick where clips should go (\"Send clips to\").");
        }
        if self.record_from == self.send_to {
            bail!("\"Record from\" and \"Send clips to\" can't be the same device.");
        }
        if self.forwards_mic() && is_virtual_cable(&self.mic) {
            bail!("\"{}\" is the virtual cable itself. Forward your real mic instead.", self.mic);
        }
        Ok(())
    }
}

fn find_output(devices: &DeviceList, words: &[&str]) -> Option<String> {
    devices.outputs.iter().find(|name| words.iter().all(|word| name.to_lowercase().contains(word))).cloned()
}

fn is_virtual_cable(name: &str) -> bool {
    let name = name.to_lowercase();
    name.contains("cable output") || name.contains("vb-audio")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn devices(outputs: &[&str], inputs: &[&str]) -> DeviceList {
        DeviceList {
            outputs: outputs.iter().map(|name| name.to_string()).collect(),
            inputs: inputs.iter().map(|name| name.to_string()).collect(),
            default_output: outputs.first().map(|name| name.to_string()),
            default_input: inputs.first().map(|name| name.to_string()),
        }
    }

    #[test]
    fn detects_goxlr_channels() {
        let found = DeviceConfig::detect(&devices(
            &["Speakers", "Chat (TC-HELICON GoXLR)", "Sample (TC-HELICON GoXLR)"],
            &["Chat Mic (TC-HELICON GoXLR)"],
        ))
        .unwrap();
        assert_eq!(found.record_from, "Chat (TC-HELICON GoXLR)");
        assert_eq!(found.send_to, "Sample (TC-HELICON GoXLR)");
        assert!(!found.forwards_mic());
        assert!(found.route() == Route::GoXlr);
    }

    #[test]
    fn detects_vb_cable_and_forwards_the_real_mic() {
        let found = DeviceConfig::detect(&devices(
            &["Headphones", "CABLE Input (VB-Audio Virtual Cable)"],
            &["CABLE Output (VB-Audio Virtual Cable)", "Microphone"],
        ))
        .unwrap();
        assert_eq!(found.record_from, "Headphones");
        assert_eq!(found.send_to, "CABLE Input (VB-Audio Virtual Cable)");
        assert_eq!(found.mic, "Microphone");
        assert!(found.forwards_mic() && found.hear_clips);
        assert!(found.validate().is_ok());
    }

    #[test]
    fn finds_nothing_without_a_virtual_mic() {
        assert!(DeviceConfig::detect(&devices(&["Speakers"], &["Microphone"])).is_none());
    }

    #[test]
    fn rejects_forwarding_the_cable_into_itself() {
        let config = DeviceConfig {
            record_from: "Headphones".into(),
            send_to: "CABLE Input".into(),
            forward_mic: true,
            mic: "CABLE Output (VB-Audio Virtual Cable)".into(),
            hear_clips: false,
        };
        assert!(config.validate().is_err());
    }
}
