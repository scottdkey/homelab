use crate::exec::local;
use anyhow::{Context, Result};

/// Detect if we're running locally on the target host or remotely
pub fn is_local_execution(hostname: &str, config: &crate::config::EnvConfig) -> Result<bool> {
    let host_config = config
        .hosts
        .get(hostname)
        .with_context(|| format!("Host '{}' not found in config", hostname))?;

    // Get target IP
    let target_ip = if let Some(ip) = &host_config.ip {
        ip.clone()
    } else {
        // If no IP configured, assume remote
        return Ok(false);
    };

    // Get local IP addresses
    let local_ips = get_local_ips()?;

    // Check if target IP matches any local IP
    Ok(local_ips.contains(&target_ip))
}

/// Get all local IP addresses
pub fn get_local_ips() -> Result<Vec<String>> {
    let mut ips = Vec::new();

    // Try to get IPs using platform-specific commands
    #[cfg(unix)]
    {
        // Use `hostname -I` on Linux or `ifconfig` on macOS
        if let Ok(output) = local::execute("hostname", &["-I"]) {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for ip in stdout.split_whitespace() {
                ips.push(ip.to_string());
            }
        }

        // Also try `ip addr` on Linux
        if let Ok(output) = local::execute("ip", &["addr", "show"]) {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("inet ") && !line.contains("127.0.0.1") && !line.contains("::1") {
                    if let Some(ip_part) = line.split_whitespace().nth(1) {
                        if let Some(ip) = ip_part.split('/').next() {
                            ips.push(ip.to_string());
                        }
                    }
                }
            }
        }
    }

    #[cfg(windows)]
    {
        // Use `ipconfig` on Windows
        if let Ok(output) = local::execute("ipconfig", &[]) {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("IPv4 Address") || line.contains("IPv4 地址") {
                    if let Some(ip_part) = line.split(':').nth(1) {
                        let ip = ip_part.trim();
                        if !ip.is_empty() {
                            ips.push(ip.to_string());
                        }
                    }
                }
            }
        }
    }

    Ok(ips)
}

pub fn print_summary(
    hostname: &str,
    target_host: &str,
    all_passed: bool,
    error_output: &[u8],
) -> Result<()> {
    let error_output_str = String::from_utf8_lossy(error_output);
    if error_output_str.contains("No errors found") || error_output_str.trim().is_empty() {
        println!("   ✓ No recent errors in OpenVPN logs");
    } else {
        println!("   ⚠ Found potential issues in logs:");
        for line in error_output_str.lines().take(5) {
            if !line.trim().is_empty() && !line.contains("No errors found") {
                println!("     - {}", line.trim());
            }
        }
    }

    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    if all_passed {
        println!("  Host: {}", hostname);
        println!("  ✓ VPN Verification Complete - All Tests Passed");
    } else {
        println!("  ⚠ VPN Verification Complete - Some Tests Failed");
    }
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
    println!(
        "VPN Status: {}",
        if all_passed {
            "OPERATIONAL"
        } else {
            "ISSUES DETECTED"
        }
    );
    println!();
    println!("Proxy Access:");
    println!("  From host: http://{}:8888", target_host);
    println!("  From containers: http://openvpn-pia:8888");
    println!();
    println!("Example usage:");
    println!(
        "  curl --proxy http://{}:8888 https://api.ipify.org",
        target_host
    );

    Ok(())
}
