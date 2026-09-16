# desktop-reminder

一个使用 Rust 编写的跨平台命令行提醒工具，支持 Linux、Windows 和 macOS。它在结束时间之前发送系统通知。

Linux 使用 Freedesktop D-Bus 通知协议，因此 KDE Plasma、GNOME 和 XFCE 共用同一套实现。Windows 使用 WinRT Toast，macOS 使用原生通知 API。`icon.png` 会在编译时嵌入可执行文件；运行时如果找到外部图标则直接使用，否则才释放内嵌图标到临时文件。

通知的显示和停留时间最终由操作系统及桌面环境决定。KDE 通常支持永久通知；Windows 和 macOS 可能会自动收起屏幕上的弹窗，但通知仍会保留在系统通知中心。

## 使用

```bash
cargo run --release -- \
  --end-time "2026-09-17 10:38:09" \
  --interval 10m \
  --title "休息一下" \
  --message "起来活动几分钟"
```

`--end-time` 使用本地时区，格式固定为 `YYYY-MM-DD HH:MM:SS`，省略时提醒永不自动结束。`--interval` 支持秒、分钟和小时，例如 `2s`、`10m`、`1h`。

程序启动后先等待一轮间隔时间，再发送第一条通知，之后每隔指定时间发送，直到结束时间或用户按下 `Ctrl+C`。需要在对应的图形用户会话中运行。

通知正文会自动附加发送次数：指定结束时间时显示为 `正文 (当前次数 / 总次数)`，省略结束时间时显示为 `正文 (当前次数)`。例如 `起来活动几分钟 (1 / 5)`。

省略结束时间可持续提醒，手动按 `Ctrl+C` 停止：

```bash
./target/release/desktop-reminder --interval 1h
```

## 构建

```bash
cargo build --release
./target/release/desktop-reminder \
  --end-time "2026-09-17 10:38:09" \
  --interval 1h
```

Windows PowerShell：

```powershell
cargo build --release
.\target\release\desktop-reminder.exe `
  --end-time "2026-09-17 10:38:09" `
  --interval 1h
```

macOS：

```bash
cargo build --release
./target/release/desktop-reminder \
  --end-time "2026-09-17 10:38:09" \
  --interval 1h
```

需要在对应的图形用户会话中运行，不能在没有桌面通知服务的纯 SSH 会话中使用。
