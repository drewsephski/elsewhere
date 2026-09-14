use codex_provider::{generate_schema_to_dir, verify_generated_schema_dir};

fn main() {
    let temp = tempfile::tempdir().expect("tempdir");
    generate_schema_to_dir(temp.path()).expect("generate schema");
    verify_generated_schema_dir(temp.path()).expect("schema compatibility");
    println!("Codex app-server schema compatibility check passed.");
    println!("Schema bundle written to: {}", temp.path().display());
}
