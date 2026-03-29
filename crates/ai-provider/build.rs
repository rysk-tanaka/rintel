fn main() {
    #[cfg(target_os = "macos")]
    {
        swift_rs::SwiftLinker::new("14")
            .with_package("AppleIntelligence", "../../apple-intelligence/")
            .link();
    }
}
