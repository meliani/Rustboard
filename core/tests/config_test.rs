use serde_yaml::Value;

#[test]
fn loads_example_config() {
    let data = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../config/services.example.yaml"));
    let services: Vec<Value> = serde_yaml::from_str(data).expect("should parse yaml");
    assert!(services.len() >= 1);
}
