# 运行

> 状态：已实现（2026-11-21，基线 v0.1.0）。

在仓库根目录直接运行 `dev.bat`（Windows）或 `./dev.sh`（macOS/Linux），会自动检查依赖并启动 `pnpm tauri dev`。也可以手动执行 `pnpm install` 后运行 `pnpm tauri dev`。环境、四个 CLI 的命令和数据库位置写在仓库根的 [README.md](../../README.md)。中文说明在 [README.zh-CN.md](../../README.zh-CN.md)。

`dev.bat --web`（或 `pnpm dev`）只打开页面，不能调用本机 CLI，也不会读写数据库。
