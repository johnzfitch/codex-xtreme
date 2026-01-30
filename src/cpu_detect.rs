use std::process::Command;

#[derive(Debug, Clone)]
pub struct CpuTarget {
    pub name: String,
    pub detected_by: DetectionMethod,
}

impl CpuTarget {
    pub fn display_name(&self) -> String {
        cpu_display_name(&self.name)
    }

    pub fn rustc_target_cpu(&self) -> &str {
        if self.name == "unknown" {
            "native"
        } else {
            &self.name
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionMethod {
    PowerShell,
    Wmic,
    Env,
    Sysctl,
    Procfs,
    Rustc,
    Fallback,
}

impl DetectionMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            DetectionMethod::PowerShell => "PowerShell",
            DetectionMethod::Wmic => "WMIC",
            DetectionMethod::Env => "Env",
            DetectionMethod::Sysctl => "Sysctl",
            DetectionMethod::Procfs => "Procfs",
            DetectionMethod::Rustc => "Rustc",
            DetectionMethod::Fallback => "Fallback",
        }
    }
}

impl std::fmt::Display for DetectionMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

pub fn detect_cpu_target() -> CpuTarget {
    if let Some((name, detected_by)) = detect_cpu_family() {
        return CpuTarget { name, detected_by };
    }

    if let Some(name) = detect_cpu_from_rustc() {
        return CpuTarget {
            name,
            detected_by: DetectionMethod::Rustc,
        };
    }

    CpuTarget {
        name: "unknown".into(),
        detected_by: DetectionMethod::Fallback,
    }
}

pub fn cpu_display_name(name: &str) -> String {
    match name {
        "znver5" => "AMD Zen 5 (Ryzen 9000 / EPYC Turin)".into(),
        "znver4" => "AMD Zen 4 (Ryzen 7000-8000 / EPYC Genoa)".into(),
        "znver3" => "AMD Zen 3 (Ryzen 5000-6000 / EPYC Milan)".into(),
        "znver2" => "AMD Zen 2 (Ryzen 3000-4000 / EPYC Rome)".into(),
        "znver1" => "AMD Zen 1 (Ryzen 1000/2000)".into(),
        "arrowlake" => "Intel Arrow Lake (15th Gen)".into(),
        "alderlake" => "Intel Alder Lake (12th Gen)".into(),
        "raptorlake" => "Intel Raptor Lake (13th/14th Gen)".into(),
        "tigerlake" => "Intel Tiger Lake (11th Gen)".into(),
        "icelake" => "Intel Ice Lake (10th Gen)".into(),
        "skylake" => "Intel Skylake (6th-9th Gen)".into(),
        "haswell" => "Intel Haswell (4th Gen)".into(),
        "apple-m1" => "Apple M1".into(),
        "apple-m2" => "Apple M2".into(),
        "apple-m3" => "Apple M3".into(),
        "apple-m4" => "Apple M4".into(),
        "x86-64-v3" => "Modern x86-64 (AVX2, ~2015+)".into(),
        "x86-64-v4" => "Recent x86-64 (AVX-512)".into(),
        "native" => "Native (auto-detect)".into(),
        "unknown" => "Unknown".into(),
        other => other.to_string(),
    }
}

fn detect_cpu_from_rustc() -> Option<String> {
    if which::which("rustc").is_err() {
        return None;
    }

    let output = Command::new("rustc")
        .args(["--print=target-cpus"])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.contains("native") && line.contains("currently") {
            if let Some(rest) = line.split("currently ").nth(1) {
                let cpu = rest.trim_end_matches(").").trim_end_matches(')').trim();
                if !cpu.is_empty() {
                    return Some(cpu.to_string());
                }
            }
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn detect_cpu_family() -> Option<(String, DetectionMethod)> {
    if let Some(name) = detect_cpu_name_powershell() {
        let cpu = map_windows_cpu_name(&name).unwrap_or_else(|| "native".into());
        return Some((cpu, DetectionMethod::PowerShell));
    }

    if let Some(name) = detect_cpu_name_wmic() {
        let cpu = map_windows_cpu_name(&name).unwrap_or_else(|| "native".into());
        return Some((cpu, DetectionMethod::Wmic));
    }

    if let Ok(proc_id) = std::env::var("PROCESSOR_IDENTIFIER") {
        let cpu = map_windows_cpu_name(&proc_id).unwrap_or_else(|| "native".into());
        return Some((cpu, DetectionMethod::Env));
    }

    None
}

#[cfg(target_os = "windows")]
fn detect_cpu_name_powershell() -> Option<String> {
    detect_cpu_name_powershell_with("powershell")
        .or_else(|| detect_cpu_name_powershell_with("pwsh"))
}

#[cfg(target_os = "windows")]
fn detect_cpu_name_powershell_with(shell: &str) -> Option<String> {
    let shell_path = which::which(shell).ok()?;
    let output = Command::new(shell_path)
        .args([
            "-NoProfile",
            "-Command",
            "(Get-CimInstance Win32_Processor).Name",
        ])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let name = stdout.lines().next()?.trim();
    // Validate output: non-empty and reasonable length
    if name.is_empty() || name.len() > 256 {
        None
    } else {
        Some(name.to_string())
    }
}

#[cfg(target_os = "windows")]
fn detect_cpu_name_wmic() -> Option<String> {
    let wmic_path = which::which("wmic").ok()?;
    let output = Command::new(wmic_path)
        .args(["cpu", "get", "name"])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("name") {
            continue;
        }
        // Validate output: reasonable length
        if trimmed.len() > 256 {
            return None;
        }
        return Some(trimmed.to_string());
    }

    None
}

#[cfg(target_os = "windows")]
fn map_windows_cpu_name(name: &str) -> Option<String> {
    let upper = name.to_ascii_uppercase();
    if upper.contains("AMD") {
        return Some(map_amd_series(&upper).unwrap_or_else(|| "native".into()));
    }

    if upper.contains("INTEL") {
        return Some(map_intel_name(&upper).unwrap_or_else(|| "native".into()));
    }

    Some("native".into())
}

#[cfg(target_os = "windows")]
fn map_amd_series(upper: &str) -> Option<String> {
    let series = extract_first_4digit_number(upper)?;
    // AMD Ryzen series mapping:
    // - 9000: Zen 5 (Granite Ridge)
    // - 8000: Zen 4 (Hawk Point, Phoenix)
    // - 7000: Zen 4 (Raphael)
    // - 6000: Zen 3+ (Rembrandt, mobile)
    // - 5000: Zen 3 (Vermeer, Cezanne)
    // - 4000: Zen 2 (Renoir, mobile)
    // - 3000: Zen 2 (Matisse) / Zen+ (some APUs)
    // - 1000-2000: Zen 1
    let cpu = match series {
        9000..=9999 => "znver5",
        8000..=8999 => "znver4",
        7000..=7999 => "znver4",
        6000..=6999 => "znver3",
        5000..=5999 => "znver3",
        4000..=4999 => "znver2",
        3000..=3999 => "znver2",
        1000..=2999 => "znver1",
        _ => "native",
    };
    Some(cpu.into())
}

#[cfg(target_os = "windows")]
fn map_intel_name(upper: &str) -> Option<String> {
    // Intel Core generation mapping
    if upper.contains("15TH GEN") || upper.contains("ARROW LAKE") {
        return Some("arrowlake".into());
    }
    if upper.contains("14TH GEN") || upper.contains("13TH GEN") {
        return Some("raptorlake".into());
    }
    if upper.contains("12TH GEN") {
        return Some("alderlake".into());
    }
    if upper.contains("11TH GEN") {
        return Some("tigerlake".into());
    }
    if upper.contains("10TH GEN") {
        return Some("icelake".into());
    }
    // 6th-9th gen all use Skylake-derived microarchitecture
    if upper.contains("9TH GEN")
        || upper.contains("8TH GEN")
        || upper.contains("7TH GEN")
        || upper.contains("6TH GEN")
    {
        return Some("skylake".into());
    }

    Some("native".into())
}

#[cfg(target_os = "windows")]
fn extract_first_4digit_number(text: &str) -> Option<u32> {
    let mut digits = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
            if digits.len() == 4 {
                return digits.parse().ok();
            }
        } else {
            digits.clear();
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn detect_cpu_family() -> Option<(String, DetectionMethod)> {
    let output = Command::new("sysctl")
        .args(["-n", "machdep.cpu.brand_string"])
        .output()
        .ok()?;

    let brand = String::from_utf8_lossy(&output.stdout);
    let upper = brand.to_ascii_uppercase();
    let name = if upper.contains("APPLE M4") {
        "apple-m4"
    } else if upper.contains("APPLE M3") {
        "apple-m3"
    } else if upper.contains("APPLE M2") {
        "apple-m2"
    } else if upper.contains("APPLE M1") {
        "apple-m1"
    } else if upper.contains("INTEL") {
        "native"
    } else {
        return None;
    };

    Some((name.into(), DetectionMethod::Sysctl))
}

#[cfg(target_os = "linux")]
fn detect_cpu_family() -> Option<(String, DetectionMethod)> {
    let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").ok()?;

    if cpuinfo.contains("AuthenticAMD") {
        let family = parse_cpuinfo_field(&cpuinfo, "cpu family")?;
        let model = parse_cpuinfo_field(&cpuinfo, "model").unwrap_or(0);

        // AMD CPU family/model mapping:
        // Family 26: Zen 5
        // Family 25: Zen 4 / Zen 3 (different models)
        // Family 24: Zen 3 (EPYC Milan)
        // Family 23: Zen 2 (model >= 49) / Zen+ (model 8-47) / Zen 1 (model 1-7)
        let znver = match family {
            26 => "znver5",
            25 => {
                // Zen 4: models 97+ (Raphael), 116+ (Phoenix)
                // Zen 3: models 0-80 (Vermeer, Cezanne)
                if model >= 97 {
                    "znver4"
                } else {
                    "znver3"
                }
            }
            24 => "znver3",
            23 => {
                // Zen 2: models 49+ (Matisse 71, Rome 49, Renoir 96)
                // Zen+: models 8-47 (Pinnacle Ridge 8, Picasso 24)
                // Zen 1: models 1-7 (Summit Ridge 1, Raven Ridge 17)
                if model >= 49 {
                    "znver2"
                } else {
                    "znver1"
                }
            }
            _ => "native",
        };

        return Some((znver.into(), DetectionMethod::Procfs));
    }

    if cpuinfo.contains("GenuineIntel") {
        if let Some(model_name) = first_model_name(&cpuinfo) {
            let upper = model_name.to_ascii_uppercase();
            if upper.contains("15TH GEN") || upper.contains("ARROW LAKE") {
                return Some(("arrowlake".into(), DetectionMethod::Procfs));
            }
            if upper.contains("14TH GEN") || upper.contains("13TH GEN") {
                return Some(("raptorlake".into(), DetectionMethod::Procfs));
            }
            if upper.contains("12TH GEN") {
                return Some(("alderlake".into(), DetectionMethod::Procfs));
            }
            if upper.contains("11TH GEN") {
                return Some(("tigerlake".into(), DetectionMethod::Procfs));
            }
            if upper.contains("10TH GEN") {
                return Some(("icelake".into(), DetectionMethod::Procfs));
            }
            if upper.contains("9TH GEN")
                || upper.contains("8TH GEN")
                || upper.contains("7TH GEN")
                || upper.contains("6TH GEN")
            {
                return Some(("skylake".into(), DetectionMethod::Procfs));
            }
        }
        return Some(("native".into(), DetectionMethod::Procfs));
    }

    None
}

#[cfg(target_os = "linux")]
fn first_model_name(cpuinfo: &str) -> Option<&str> {
    for line in cpuinfo.lines() {
        if let Some(rest) = line.strip_prefix("model name") {
            let name = rest.split(':').nth(1)?.trim();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn parse_cpuinfo_field(cpuinfo: &str, field: &str) -> Option<u32> {
    for line in cpuinfo.lines() {
        if line.starts_with(field) {
            // Format: "cpu family\t: 23" or "model\t\t: 113"
            let value = line.split(':').nth(1)?.trim();
            return value.parse().ok();
        }
    }
    None
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn detect_cpu_family() -> Option<(String, DetectionMethod)> {
    None
}
