use crate::error::{AppError, AppResult};
use base64::{engine::general_purpose::STANDARD, Engine};
use html5ever::tokenizer::{BufferQueue, StartTag, Token, TokenSink, TokenSinkResult, Tokenizer};
use std::{
    cell::RefCell,
    process::{Command, Stdio},
};
use std::{
    io::Read,
    time::{Duration, Instant},
};

const ICON_LIMIT: usize = 256 * 1024;
fn unavailable() -> AppError {
    AppError::new(
        "ICON_UNAVAILABLE",
        "未获取到网站图标，可使用默认图标或稍后重试",
    )
}
fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0, 0, 1, 0]) {
        Some("image/x-icon")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some("image/webp")
    } else {
        let text = std::str::from_utf8(bytes)
            .ok()?
            .trim_start_matches('\u{feff}')
            .trim_start();
        if text.starts_with("<svg") || (text.starts_with("<?xml") && text.contains("<svg")) {
            Some("image/svg+xml")
        } else {
            None
        }
    }
}
pub fn valid_icon_data(value: &str) -> bool {
    if value.len() > 350_000 {
        return false;
    }
    let Some((header, encoded)) = value.split_once(',') else {
        return false;
    };
    let Ok(bytes) = STANDARD.decode(encoded) else {
        return false;
    };
    !bytes.is_empty()
        && bytes.len() <= ICON_LIMIT
        && image_mime(&bytes).is_some_and(|mime| header == format!("data:{mime};base64"))
}
fn read_resource(url: &str, limit: usize, deadline: Instant) -> AppResult<(String, Vec<u8>)> {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .ok_or_else(unavailable)?;
    let url = super::projects::validated_url(url)?;
    #[cfg(target_os = "windows")]
    let executable = std::path::PathBuf::from(
        std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()),
    )
    .join("System32/curl.exe");
    #[cfg(not(target_os = "windows"))]
    let executable = std::path::PathBuf::from("/usr/bin/curl");
    let mut command = Command::new(executable);
    command
        .args([
            "--disable",
            "--silent",
            "--fail",
            "--location",
            "--max-redirs",
            "5",
            "--proto",
            "=http,https",
            "--proto-redir",
            "=http,https",
            "--connect-timeout",
            "3",
            "--max-time",
            &format!("{:.3}", remaining.as_secs_f64().min(6.0)),
            "--max-filesize",
            &limit.to_string(),
            "--user-agent",
            "ProjectManage/0.1 favicon",
            "--write-out",
            "\n%{url_effective}",
            "--url",
            &url,
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(Stdio::piped());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn().map_err(|_| unavailable())?;
    let mut bytes = Vec::new();
    let read = child
        .stdout
        .take()
        .ok_or_else(unavailable)?
        .take(limit as u64 + 4096)
        .read_to_end(&mut bytes);
    if read.is_err() || bytes.len() > limit + 2049 {
        let _ = child.kill();
        let _ = child.wait();
        return Err(unavailable());
    }
    if !child.wait().map_err(|_| unavailable())?.success() {
        return Err(unavailable());
    }
    let separator = bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .ok_or_else(unavailable)?;
    let final_url = std::str::from_utf8(&bytes[separator + 1..])
        .map_err(|_| unavailable())?
        .to_string();
    super::projects::validated_url(&final_url)?;
    bytes.truncate(separator);
    if bytes.len() > limit {
        return Err(unavailable());
    }
    Ok((final_url, bytes))
}
#[derive(Default)]
struct IconTags {
    base: RefCell<Option<String>>,
    icons: RefCell<Vec<String>>,
}
impl TokenSink for IconTags {
    type Handle = ();
    fn process_token(&self, token: Token, _: u64) -> TokenSinkResult<()> {
        if let Token::TagToken(tag) = token {
            if tag.kind == StartTag {
                let attr = |name: &str| {
                    tag.attrs
                        .iter()
                        .find(|a| a.name.local.as_ref() == name)
                        .map(|a| a.value.to_string())
                };
                match tag.name.as_ref() {
                    "script" => {
                        return TokenSinkResult::RawData(
                            html5ever::tokenizer::states::RawKind::ScriptData,
                        )
                    }
                    "style" => {
                        return TokenSinkResult::RawData(
                            html5ever::tokenizer::states::RawKind::Rawtext,
                        )
                    }
                    "title" | "textarea" => {
                        return TokenSinkResult::RawData(
                            html5ever::tokenizer::states::RawKind::Rcdata,
                        )
                    }
                    "base" if self.base.borrow().is_none() => {
                        *self.base.borrow_mut() = attr("href")
                    }
                    "link" => {
                        let rel = attr("rel").unwrap_or_default();
                        if rel.split_ascii_whitespace().any(|r| {
                            r.eq_ignore_ascii_case("icon")
                                || r.eq_ignore_ascii_case("apple-touch-icon")
                        }) {
                            if let Some(href) = attr("href") {
                                if self.icons.borrow().len() < 4 {
                                    self.icons.borrow_mut().push(href);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        TokenSinkResult::Continue
    }
}
fn icon_candidates(page_url: &str, html: &[u8]) -> Vec<String> {
    let Ok(page) = tauri::Url::parse(page_url) else {
        return Vec::new();
    };
    let input = BufferQueue::default();
    input.push_back(String::from_utf8_lossy(html).as_ref().into());
    let tokenizer = Tokenizer::new(IconTags::default(), Default::default());
    let _ = tokenizer.feed(&input);
    tokenizer.end();
    let tags = tokenizer.sink;
    let base = tags
        .base
        .borrow()
        .as_ref()
        .and_then(|href| page.join(href).ok())
        .unwrap_or(page.clone());
    let mut urls = Vec::new();
    for href in tags.icons.into_inner() {
        if let Ok(url) = base.join(&href) {
            if super::projects::validated_url(url.as_str()).is_ok()
                && !urls.contains(&url.to_string())
            {
                urls.push(url.to_string());
            }
        }
    }
    for path in ["/favicon.ico", "/apple-touch-icon.png"] {
        if let Ok(url) = page.join(path) {
            if !urls.contains(&url.to_string()) {
                urls.push(url.to_string());
            }
        }
    }
    urls
}
pub fn extract(raw: &str) -> AppResult<String> {
    let url = super::projects::validated_url(raw)?;
    let deadline = Instant::now() + Duration::from_secs(12);
    let (page_url, html) = read_resource(&url, 512 * 1024, deadline).unwrap_or((url, Vec::new()));
    for candidate in icon_candidates(&page_url, &html) {
        if let Ok((_, bytes)) = read_resource(&candidate, ICON_LIMIT, deadline) {
            if let Some(mime) = image_mime(&bytes) {
                return Ok(format!("data:{mime};base64,{}", STANDARD.encode(&bytes)));
            }
        }
    }
    Err(unavailable())
}
