use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use clap::Parser;
use notify_rust::{Notification, Timeout};

const END_TIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";
const EMBEDDED_ICON: &[u8] = include_bytes!("../icon.png");

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
compile_error!("desktop-reminder supports Linux, Windows, and macOS only");

#[derive(Debug, Parser)]
#[command(
    name = "desktop-reminder",
    version,
    about = "按固定间隔发送桌面系统通知，可选择指定结束时间"
)]
struct Args {
    /// 结束时间，格式为 YYYY-MM-DD HH:MM:SS，例如 2026-09-17 10:38:09；省略则永不结束
    #[arg(long, value_name = "DATETIME")]
    end_time: Option<String>,

    /// 通知间隔，例如 2s、10m、1h
    #[arg(long, value_name = "DURATION")]
    interval: String,

    /// 通知标题
    #[arg(long, default_value = "定时提醒")]
    title: String,

    /// 通知内容
    #[arg(long, default_value = "提醒时间到了")]
    message: String,
}

fn parse_end_time(value: &str) -> Result<DateTime<Local>> {
    let naive = NaiveDateTime::parse_from_str(value, END_TIME_FORMAT)
        .with_context(|| format!("无效的结束时间 {value:?}，格式必须是 YYYY-MM-DD HH:MM:SS"))?;

    Local
        .from_local_datetime(&naive)
        .single()
        .with_context(|| format!("结束时间 {value:?} 在当前时区不存在或有歧义"))
}

fn parse_interval(value: &str) -> Result<Duration> {
    if value.len() < 2 {
        bail!("无效的间隔 {value:?}，请使用类似 2s、10m 或 1h 的格式");
    }

    let (number, unit) = value.split_at(value.len() - 1);
    let amount: u64 = number
        .parse()
        .with_context(|| format!("间隔数值无效: {number:?}"))?;
    if amount == 0 {
        bail!("间隔必须大于零");
    }

    let seconds = match unit {
        "s" => amount,
        "m" => amount.checked_mul(60).context("间隔太大")?,
        "h" => amount
            .checked_mul(60)
            .and_then(|seconds| seconds.checked_mul(60))
            .context("间隔太大")?,
        _ => bail!("不支持的间隔单位 {unit:?}，只支持 s、m、h"),
    };

    Ok(Duration::from_secs(seconds))
}

struct EmbeddedIcon {
    path: PathBuf,
    temporary: bool,
}

impl Drop for EmbeddedIcon {
    fn drop(&mut self) {
        if self.temporary {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn prepare_icon() -> Result<EmbeddedIcon> {
    let mut external_candidates = vec![
        PathBuf::from("desktop-reminder/icon.png"),
        PathBuf::from("icon.png"),
    ];
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            external_candidates.push(directory.join("icon.png"));
        }
    }

    if let Some(path) = external_candidates.into_iter().find(|path| path.is_file()) {
        return Ok(EmbeddedIcon {
            path: path
                .canonicalize()
                .with_context(|| format!("解析图标路径失败: {}", path.display()))?,
            temporary: false,
        });
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "desktop-reminder-icon-{}-{timestamp}.png",
        std::process::id()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .with_context(|| format!("创建临时图标文件失败: {}", path.display()))?;

    if let Err(error) = file.write_all(EMBEDDED_ICON) {
        let _ = fs::remove_file(&path);
        return Err(error).context("写入嵌入式图标失败");
    }

    Ok(EmbeddedIcon {
        path,
        temporary: true,
    })
}

fn show_notification(title: &str, message: &str, icon_path: &Path) -> Result<()> {
    let icon_path = icon_path.to_str().context("临时图标路径不是有效的 UTF-8")?;
    let icon_uri = format!(
        "file://{}",
        icon_path
            .replace('%', "%25")
            .replace(' ', "%20")
            .replace('#', "%23")
            .replace('?', "%3F")
    );

    Notification::new()
        .appname("desktop-reminder")
        .summary(title)
        .body(message)
        .icon(&icon_uri)
        .image_path(icon_path)
        .timeout(Timeout::Never)
        .show()
        .context("发送桌面系统通知失败")?;
    Ok(())
}

fn run(args: Args) -> Result<()> {
    let end_time = args.end_time.as_deref().map(parse_end_time).transpose()?;
    let interval = parse_interval(&args.interval)?;
    let icon = prepare_icon()?;

    if let Some(end_time) = end_time {
        if end_time <= Local::now() {
            bail!("结束时间必须晚于当前时间");
        }

        println!(
            "将每 {} 秒发送一次通知，直到 {}（本地时间）；按 Ctrl+C 可退出。",
            interval.as_secs(),
            end_time.format(END_TIME_FORMAT)
        );
    } else {
        println!(
            "将每 {} 秒持续发送通知，不会自动结束；按 Ctrl+C 可退出。",
            interval.as_secs()
        );
    }

    loop {
        if let Some(end_time) = end_time {
            let remaining = (end_time - Local::now()).to_std().unwrap_or(Duration::ZERO);
            if remaining < interval {
                break;
            }
        }

        thread::sleep(interval);
        if end_time.is_some_and(|end_time| Local::now() >= end_time) {
            break;
        }

        show_notification(&args.title, &args.message, &icon.path)?;
    }

    if end_time.is_some() {
        println!("已到达结束时间，提醒结束。");
    }
    Ok(())
}

fn main() -> Result<()> {
    run(Args::parse())
}

#[cfg(test)]
mod tests {
    use super::{parse_end_time, parse_interval};
    use std::time::Duration;

    #[test]
    fn parses_supported_intervals() {
        assert_eq!(parse_interval("2s").unwrap(), Duration::from_secs(2));
        assert_eq!(parse_interval("10m").unwrap(), Duration::from_secs(600));
        assert_eq!(parse_interval("1h").unwrap(), Duration::from_secs(3600));
    }

    #[test]
    fn rejects_invalid_intervals() {
        assert!(parse_interval("0s").is_err());
        assert!(parse_interval("1d").is_err());
        assert!(parse_interval("1.5s").is_err());
    }

    #[test]
    fn parses_local_end_time() {
        let time = parse_end_time("2026-09-17 10:38:09").unwrap();
        assert_eq!(
            time.format("%Y-%m-%d %H:%M:%S").to_string(),
            "2026-09-17 10:38:09"
        );
    }
}
