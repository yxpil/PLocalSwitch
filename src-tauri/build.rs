fn main() {
    // Tauri 构建脚本：生成图标、权限等（仅 feature=desktop-shell 时启用）
    // 纯 axum 网关二进制无需 tauri_build，跳过即可。
    #[cfg(feature = "desktop-shell")]
    tauri_build::build();
}
