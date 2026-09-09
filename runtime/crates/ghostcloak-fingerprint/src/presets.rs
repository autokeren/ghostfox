//! Coherent hardware/software persona pools. These are the "real device"
//! combinations a generator draws from — every field chosen so the whole
//! identity reads like one physical machine, not a pile of random values.

use crate::identity::{Hardware, Platform, Screen};

/// A coherent device persona: platform + screen + memory + GPU + fonts that
/// actually ship together in the wild.
pub struct DevicePreset {
    pub platform: Platform,
    pub ua: &'static str,
    pub screen: (u32, u32, f64),
    pub cores: u32,
    pub memory_gb: u32,
    pub gpu: &'static str,
}

/// GPUs commonly seen on Windows desktops (via ANGLE, D3D11).
const WIN_GBUS: &[&str] = &[
    "ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 (0x00002503) Direct3D11 vs_5_0 ps_5_0, D3D11)",
    "ANGLE (NVIDIA, NVIDIA GeForce RTX 4070 (0x00002786) Direct3D11 vs_5_0 ps_5_0, D3D11)",
    "ANGLE (Intel, Intel(R) UHD Graphics 630 (0x00003E92) Direct3D11 vs_5_0 ps_5_0, D3D11)",
    "ANGLE (AMD, AMD Radeon RX 6700 XT (0x000073BF) Direct3D11 vs_5_0 ps_5_0, D3D11)",
];

/// Apple Silicon Macs share one GPU vendor string pattern.
const MAC_GBUS: &[&str] = &[
    "ANGLE (Apple, ANGLE Metal Renderer: Apple M2, Unspecified Version)",
    "ANGLE (Apple, ANGLE Metal Renderer: Apple M3 Pro, Unspecified Version)",
    "ANGLE (Apple, ANGLE Metal Renderer: Apple M1, Unspecified Version)",
];

/// Linux desktops.
const LINUX_GBUS: &[&str] = &[
    "ANGLE (NVIDIA, NVIDIA GeForce RTX 3060/PCIe/SSE2, OpenGL 4.5)",
    "ANGLE (Intel, Mesa Intel(R) UHD Graphics 730 (ADL-S GT1), OpenGL 4.6)",
    "ANGLE (AMD, AMD Radeon RX 6700 XT (radeonsi navi22 LLVM 15.0.7), OpenGL 4.6)",
];

/// Android GPUs (Firefox on Android reports the raw renderer, no ANGLE).
const ANDROID_GBUS: &[&str] = &[
    "Adreno (TM) 740",
    "Adreno (TM) 750",
    "Mali-G715-Immortalis MC11",
    "Mali-G615",
    "Adreno (TM) 730",
];

pub const WIN_FONTS: &[&str] = &[
    "Arial",
    "Arial Black",
    "Calibri",
    "Cambria",
    "Candara",
    "Comic Sans MS",
    "Consolas",
    "Constantia",
    "Corbel",
    "Courier New",
    "Ebrima",
    "Franklin Gothic Medium",
    "Gabriola",
    "Georgia",
    "Impact",
    "Javanese Text",
    "Lucida Console",
    "Lucida Sans Unicode",
    "Malgun Gothic",
    "Marlett",
    "Microsoft Himalaya",
    "Microsoft JhengHei",
    "Microsoft New Tai Lue",
    "Microsoft Sans Serif",
    "Microsoft Tai Le",
    "Mongolian Baiti",
    "MS Gothic",
    "MV Boli",
    "Nirmala UI",
    "Palatino Linotype",
    "Segoe Print",
    "Segoe Script",
    "Segoe UI",
    "Segoe UI Emoji",
    "Segoe UI Historic",
    "Segoe UI Symbol",
    "SimSun",
    "Sitka",
    "Sylfaen",
    "Symbol",
    "Tahoma",
    "Times New Roman",
    "Trebuchet MS",
    "Verdana",
    "Webdings",
    "Wingdings",
    "Yu Gothic",
];

pub const MAC_FONTS: &[&str] = &[
    "Al Bayan",
    "American Typewriter",
    "Andale Mono",
    "Apple Color Emoji",
    "AppleGothic",
    "Arial",
    "Arial Hebrew",
    "Arial Rounded MT Bold",
    "Avenir",
    "Avenir Next",
    "Baskerville",
    "Bodoni 72",
    "Bradley Hand",
    "Brush Script MT",
    "Chalkboard",
    "Chalkduster",
    "Charter",
    "Cochin",
    "Comic Sans MS",
    "Copperplate",
    "Courier New",
    "Futura",
    "Geneva",
    "Georgia",
    "Gill Sans",
    "Helvetica",
    "Helvetica Neue",
    "Herculanum",
    "Hoefler Text",
    "Impact",
    "Lucida Grande",
    "Luminari",
    "Marker Felt",
    "Menlo",
    "Microsoft Sans Serif",
    "Monaco",
    "Mukta Mahee",
    "Noteworthy",
    "Optima",
    "Palatino",
    "Papyrus",
    "Phosphate",
    "Rockwell",
    "Savoye LET",
    "SignPainter",
    "Skia",
    "Snell Roundhand",
    "Stencil",
    "Syilahar New",
    "Tahoma",
    "Times",
    "Times New Roman",
    "Trattatello",
    "Trebuchet MS",
    "Verdana",
    "Zapfino",
];

pub const LINUX_FONTS: &[&str] = &[
    "DejaVu Sans",
    "DejaVu Sans Mono",
    "DejaVu Serif",
    "FreeMono",
    "FreeSans",
    "FreeSerif",
    "Liberation Mono",
    "Liberation Sans",
    "Liberation Serif",
    "Liberation Sans Narrow",
    "Nimbus Sans",
    "Nimbus Mono",
    "Nimbus Roman",
    "Noto Sans",
    "Noto Sans Mono",
    "Noto Serif",
    "Ubuntu",
    "Ubuntu Mono",
    "Ubuntu Condensed",
    "Cantarell",
    "Cousine",
    "Tinos",
    "Arimo",
];

pub const ANDROID_FONTS: &[&str] = &[
    "Roboto",
    "Roboto Condensed",
    "Roboto Mono",
    "Noto Sans",
    "Noto Sans Arabic",
    "Noto Sans Bengali",
    "Noto Sans CJK",
    "Noto Sans Devanagari",
    "Noto Sans Thai",
    "Noto Serif",
    "Noto Color Emoji",
    "Droid Sans Mono",
    "Coming Soon",
    "Carrois Gothic SC",
];

impl DevicePreset {
    pub fn hardware(&self) -> Hardware {
        Hardware {
            cpu_cores: self.cores,
            device_memory_gb: self.memory_gb,
            gpu_renderer: self.gpu.to_string(),
            fonts: self.fonts().iter().map(|s| s.to_string()).collect(),
        }
    }

    pub fn screen(&self) -> Screen {
        Screen {
            width: self.screen.0,
            height: self.screen.1,
            dpr: self.screen.2,
        }
    }

    fn fonts(&self) -> &'static [&'static str] {
        match self.platform {
            Platform::Windows => WIN_FONTS,
            Platform::MacOS => MAC_FONTS,
            Platform::Linux => LINUX_FONTS,
            Platform::Android => ANDROID_FONTS,
        }
    }
}

pub const PRESETS: &[DevicePreset] = &[
    // Windows 11 desktops / laptops
    DevicePreset { platform: Platform::Windows, ua: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36", screen: (1920, 1080, 1.0), cores: 12, memory_gb: 8, gpu: WIN_GBUS[0] },
    DevicePreset { platform: Platform::Windows, ua: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36", screen: (2560, 1440, 1.0), cores: 16, memory_gb: 8, gpu: WIN_GBUS[1] },
    DevicePreset { platform: Platform::Windows, ua: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36", screen: (1536, 864, 1.25), cores: 8, memory_gb: 8, gpu: WIN_GBUS[2] },
    DevicePreset { platform: Platform::Windows, ua: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36", screen: (1920, 1080, 1.0), cores: 12, memory_gb: 8, gpu: WIN_GBUS[3] },
    // MacBooks
    DevicePreset { platform: Platform::MacOS, ua: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36", screen: (1512, 982, 2.0), cores: 8, memory_gb: 8, gpu: MAC_GBUS[2] },
    DevicePreset { platform: Platform::MacOS, ua: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36", screen: (1728, 1117, 2.0), cores: 10, memory_gb: 8, gpu: MAC_GBUS[0] },
    DevicePreset { platform: Platform::MacOS, ua: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36", screen: (1800, 1169, 2.0), cores: 12, memory_gb: 8, gpu: MAC_GBUS[1] },
    // Linux desktops
    DevicePreset { platform: Platform::Linux, ua: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36", screen: (1920, 1080, 1.0), cores: 8, memory_gb: 8, gpu: LINUX_GBUS[0] },
    DevicePreset { platform: Platform::Linux, ua: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36", screen: (1366, 768, 1.0), cores: 4, memory_gb: 8, gpu: LINUX_GBUS[1] },
    DevicePreset { platform: Platform::Linux, ua: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36", screen: (2560, 1440, 1.0), cores: 16, memory_gb: 8, gpu: LINUX_GBUS[2] },
    // Android phones — Firefox on Android personas. Screens are CSS pixels,
    // portrait (h > w), dpr >= 2.
    DevicePreset { platform: Platform::Android, ua: "Mozilla/5.0 (Android 15; Mobile; rv:141.0) Gecko/141.0 Firefox/141.0", screen: (412, 915, 2.625), cores: 8, memory_gb: 8, gpu: ANDROID_GBUS[0] },
    DevicePreset { platform: Platform::Android, ua: "Mozilla/5.0 (Android 14; Mobile; rv:141.0) Gecko/141.0 Firefox/141.0", screen: (384, 832, 3.0), cores: 8, memory_gb: 8, gpu: ANDROID_GBUS[1] },
    DevicePreset { platform: Platform::Android, ua: "Mozilla/5.0 (Android 15; Mobile; rv:140.0) Gecko/140.0 Firefox/140.0", screen: (393, 873, 2.75), cores: 8, memory_gb: 8, gpu: ANDROID_GBUS[2] },
    DevicePreset { platform: Platform::Android, ua: "Mozilla/5.0 (Android 14; Mobile; rv:140.0) Gecko/140.0 Firefox/140.0", screen: (360, 800, 2.8), cores: 8, memory_gb: 8, gpu: ANDROID_GBUS[3] },
    DevicePreset { platform: Platform::Android, ua: "Mozilla/5.0 (Android 13; Mobile; rv:141.0) Gecko/141.0 Firefox/141.0", screen: (412, 892, 2.625), cores: 8, memory_gb: 8, gpu: ANDROID_GBUS[4] },
];
