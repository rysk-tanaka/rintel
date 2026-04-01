fn main() {
    // Swift Concurrency ランタイム dylib を実行時に解決できるよう rpath を追加
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("xcode-select")
            .arg("-p")
            .output()
            && output.status.success()
            && let Ok(developer_dir) = String::from_utf8(output.stdout)
        {
            let developer_dir = developer_dir.trim();
            if !developer_dir.is_empty() {
                println!(
                    "cargo:rustc-link-arg=-Wl,-rpath,{developer_dir}/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift/macosx"
                );
            }
        }
    }
}
