use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{Device, DevicesError};

#[derive(Default)]
pub struct DeviceList {
    pub outputs: Vec<String>,
    pub inputs: Vec<String>,
    pub default_output: Option<String>,
    pub default_input: Option<String>,
}

impl DeviceList {
    pub fn scan() -> Self {
        let host = cpal::default_host();
        Self {
            outputs: names(host.output_devices()),
            inputs: names(host.input_devices()),
            default_output: host.default_output_device().map(|device| device_name(&device)),
            default_input: host.default_input_device().map(|device| device_name(&device)),
        }
    }
}

pub fn find_output(name: &str) -> Option<Device> {
    cpal::default_host().output_devices().ok()?.find(|device| device_name(device) == name)
}

pub fn find_input(name: &str) -> Option<Device> {
    cpal::default_host().input_devices().ok()?.find(|device| device_name(device) == name)
}

fn names(devices: Result<impl Iterator<Item = Device>, DevicesError>) -> Vec<String> {
    devices.map(|devices| devices.map(|device| device_name(&device)).collect()).unwrap_or_default()
}

fn device_name(device: &Device) -> String {
    device.name().unwrap_or_else(|_| "Unknown device".to_owned())
}
