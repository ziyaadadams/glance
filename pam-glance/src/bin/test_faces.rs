use std::env;
use std::fs;

fn main() {
    let username = env::var("USER").unwrap_or_else(|_| "ziyaadsmada".to_string());
    println!("Testing face data loading for user: {}", username);
    
    let home = env::var("HOME").unwrap_or_else(|_| "/home/ziyaadsmada".to_string());
    let paths = vec![
        format!("{}/.local/share/glance/{}.json", home, username),
        format!("{}/.config/glance/{}.json", home, username),
        format!("/var/lib/glance/{}.json", username),
    ];
    
    for path in &paths {
        if std::path::Path::new(path).exists() {
            println!("✓ Found: {}", path);
            if let Ok(content) = fs::read_to_string(path) {
                let json: serde_json::Value = serde_json::from_str(&content).unwrap();
                if let Some(encodings) = json.get("encodings").and_then(|e| e.as_array()) {
                    println!("  - Encodings count: {}", encodings.len());
                    if let Some(first) = encodings.first().and_then(|e| e.as_array()) {
                        println!("  - First encoding length: {} elements", first.len());
                    }
                }
                if let Some(username) = json.get("username") {
                    println!("  - Username: {}", username);
                }
            }
        } else {
            println!("✗ Not found: {}", path);
        }
    }
}
