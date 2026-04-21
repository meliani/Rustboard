pub trait DashboardPlugin {
    fn name(&self) -> &'static str;
    fn execute(&self, args: &str) -> String;
}

#[cfg(test)]
mod tests {
    use super::*;
    struct ExamplePlugin;
    impl DashboardPlugin for ExamplePlugin {
        fn name(&self) -> &'static str { "example" }
        fn execute(&self, args: &str) -> String { format!("executed: {}", args) }
    }

    #[test]
    fn test_example_plugin() {
        let p = ExamplePlugin;
        assert_eq!(p.name(), "example");
        assert_eq!(p.execute("x"), "executed: x");
    }
}
