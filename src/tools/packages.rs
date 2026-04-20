use super::{ToolArgs, ToolResult};
use tokio::process::Command;

pub async fn get_package_manager(_args: ToolArgs) -> ToolResult {
    // (name, args to get version output)
    let managers: &[(&str, &[&str])] = &[
        // System
        ("pacman", &["--version"]),
        ("apt", &["--version"]),
        ("apt-get", &["--version"]),
        ("dnf", &["--version"]),
        ("yum", &["--version"]),
        ("zypper", &["--version"]),
        ("apk", &["--version"]),
        ("brew", &["--version"]),
        ("nix", &["--version"]),
        ("emerge", &["--version"]),
        // AUR helpers
        ("yay", &["--version"]),
        ("paru", &["--version"]),
        ("pikaur", &["--version"]),
        // JS
        ("npm", &["--version"]),
        ("pnpm", &["--version"]),
        ("yarn", &["--version"]),
        ("bun", &["--version"]),
        // Rust
        ("cargo", &["--version"]),
        // Python
        ("pip", &["--version"]),
        ("pip3", &["--version"]),
        ("uv", &["--version"]),
        ("poetry", &["--version"]),
        ("pipenv", &["--version"]),
        ("conda", &["--version"]),
        // Other
        ("gem", &["--version"]),
        ("composer", &["--version"]),
        ("go", &["version"]),
    ];

    let mut found = Vec::new();

    for (name, args) in managers {
        if let Ok(out) = Command::new(name).args(*args).output().await {
            if out.status.success() || !out.stdout.is_empty() {
                let version = String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                found.push(format!("{name}: {version}"));
            }
        }
    }

    if found.is_empty() {
        Ok("No package managers found".to_string())
    } else {
        Ok(found.join("\n"))
    }
}
