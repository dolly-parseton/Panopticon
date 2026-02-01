use std::fs;
use std::path::Path;

fn main() {
    // Tell Cargo to rerun this script if Cargo.toml changes (tera version updates)
    println!("cargo:rerun-if-changed=Cargo.toml");

    let dest_path = Path::new("src").join("tera.pest");

    // If the pest file already exists (e.g. in a published crate), skip everything.
    // Only parse the version and download when the file is missing.
    if !dest_path.exists() {
        // Parse Cargo.toml to get tera version
        let manifest = fs::read_to_string("Cargo.toml").expect("Failed to read Cargo.toml");

        let tera_version =
            extract_tera_version(&manifest).expect("Failed to find tera version in Cargo.toml");

        let pest_url = format!(
            "https://raw.githubusercontent.com/Keats/tera/v{}/src/parser/tera.pest",
            tera_version
        );
        println!(
            "cargo:warning=Downloading tera.pest for version {}",
            tera_version
        );

        let client = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(30))
            .build();

        let response = client
            .get(&pest_url)
            .call()
            .unwrap_or_else(|_| panic!("Failed to download pest file from {}", pest_url));

        let pest_content = response
            .into_string()
            .unwrap_or_else(|_| panic!("Failed to read response body from {}", pest_url));
        let content_with_version = format!(
            "// Downloaded from tera v{}\n// https://github.com/Keats/tera/blob/v{}/src/parser/tera.pest\n\n{}",
            tera_version, tera_version, pest_content
        );

        fs::write(&dest_path, content_with_version)
            .unwrap_or_else(|_| panic!("Failed to write tera.pest to {:?}", dest_path));

        println!("cargo:warning=Successfully downloaded tera.pest");
    }
}

fn extract_tera_version(manifest: &str) -> Option<String> {
    for line in manifest.lines() {
        let line = line.trim();

        if line.starts_with("tera")
            && line.contains("=")
            && let Some(version_part) = line.split('=').nth(1)
        {
            let version_part = version_part.trim();

            if version_part.starts_with('"') && version_part.contains('"') {
                let version = version_part.trim_matches('"').trim();
                return Some(version.to_string());
            }

            if version_part.starts_with('{')
                && version_part.contains("version")
                && let Some(version_str) = version_part.split("version").nth(1)
                && let Some(quoted) = version_str.split('"').nth(1)
            {
                return Some(quoted.trim().to_string());
            }
        }
    }

    None
}
