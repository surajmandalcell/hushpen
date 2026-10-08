//! SMAppService on macOS 13+. An unsigned debug build may fail to register;
//! the error text then tells the user to allow the item in System Settings.

// A message to a framework class that the objc2 bindings of this workspace do not wrap.
#![allow(unsafe_code)]

use objc2_service_management::{SMAppService, SMAppServiceStatus};

#[derive(Debug, Clone, Copy, Default)]
pub struct MacLoginItem;

impl MacLoginItem {
    pub fn current() -> std::io::Result<Self> {
        Ok(Self)
    }

    pub fn is_enabled(&self) -> bool {
        let service = unsafe { SMAppService::mainAppService() };
        let status = unsafe { service.status() };
        status == SMAppServiceStatus::Enabled || status == SMAppServiceStatus::RequiresApproval
    }

    pub fn set(&self, enabled: bool) -> std::io::Result<()> {
        let service = unsafe { SMAppService::mainAppService() };
        let result = unsafe {
            match enabled {
                true => service.registerAndReturnError(),
                false => service.unregisterAndReturnError(),
            }
        };
        result.map_err(|error| {
            std::io::Error::other(format!("SMAppService: {}", error.localizedDescription()))
        })
    }
}
