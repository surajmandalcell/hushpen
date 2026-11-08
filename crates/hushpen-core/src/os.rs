//! Platform facts shared by the app, settings, and workers.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Os {
    Mac,
    Linux,
    Windows,
}

impl Os {
    pub const fn current() -> Self {
        #[cfg(target_os = "macos")]
        {
            Self::Mac
        }
        #[cfg(target_os = "linux")]
        {
            Self::Linux
        }
        #[cfg(target_os = "windows")]
        {
            Self::Windows
        }
    }

    pub const fn capabilities(self, x11: bool, compositor: bool) -> Capabilities {
        match self {
            Self::Mac => Capabilities {
                page_out: false,
                metal: true,
                global_keys: true,
                paste: true,
                translucent: true,
            },
            Self::Linux => Capabilities {
                page_out: true,
                metal: false,
                global_keys: x11,
                paste: x11,
                translucent: compositor,
            },
            Self::Windows => Capabilities {
                page_out: false,
                metal: false,
                global_keys: true,
                paste: true,
                translucent: compositor,
            },
        }
    }
}

/// OS support combined with facts supplied by the native backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    pub page_out: bool,
    pub metal: bool,
    pub global_keys: bool,
    pub paste: bool,
    pub translucent: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_and_gpu_support_are_explicit_on_every_os() {
        for (os, page_out, metal) in [
            (Os::Mac, false, true),
            (Os::Linux, true, false),
            (Os::Windows, false, false),
        ] {
            let caps = os.capabilities(false, false);
            assert_eq!((caps.page_out, caps.metal), (page_out, metal));
        }
    }

    #[test]
    fn linux_needs_x11_for_keys_and_paste_and_a_compositor_for_transparency() {
        for x11 in [false, true] {
            for compositor in [false, true] {
                let caps = Os::Linux.capabilities(x11, compositor);
                assert_eq!(
                    (caps.global_keys, caps.paste, caps.translucent),
                    (x11, x11, compositor)
                );
                assert!(Os::Mac.capabilities(x11, compositor).translucent);
                assert_eq!(
                    Os::Windows.capabilities(x11, compositor).translucent,
                    compositor
                );
            }
        }
    }
}
