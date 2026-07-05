use anyhow::{anyhow, Context, Result};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::Page;
use futures::StreamExt;
use once_cell::sync::Lazy;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

/// A live Chromium session shared across MCP tool calls so the user can
/// interact with the page (CAPTCHA, login) between `browser_open_url` and
/// `browser_capture_markdown`.
pub struct BrowserSession {
    browser: Browser,
    handler_task: JoinHandle<()>,
    page: Page,
    user_data_dir: std::path::PathBuf,
}

static SESSION: Lazy<Mutex<Option<BrowserSession>>> = Lazy::new(|| Mutex::new(None));

pub const DEFAULT_NAV_TIMEOUT_SECS: u64 = 30;

#[derive(Debug)]
pub struct PageInfo {
    pub url: String,
    pub title: Option<String>,
}

fn chrome_not_found_hint(err: impl std::fmt::Display) -> anyhow::Error {
    anyhow!(
        "Failed to start Chromium: {}. Install Google Chrome or Chromium \
         (e.g. \"/Applications/Google Chrome.app\") or set the CHROME environment \
         variable to the browser executable path.",
        err
    )
}

async fn launch(visible: bool, user_agent: Option<&str>) -> Result<BrowserSession> {
    // Unique profile dir per session: avoids Chrome's ProcessSingleton lock
    // colliding with the user's running Chrome or a previous crashed session.
    let user_data_dir =
        std::env::temp_dir().join(format!("to_markdown_mcp-browser-{}", uuid::Uuid::new_v4()));

    let mut builder = BrowserConfig::builder().user_data_dir(&user_data_dir);
    if visible {
        builder = builder.with_head();
    }
    let config = builder.build().map_err(chrome_not_found_hint)?;

    let (browser, mut handler) = Browser::launch(config)
        .await
        .map_err(chrome_not_found_hint)?;

    // Drive the CDP event loop for the lifetime of the browser.
    let handler_task = tokio::spawn(async move {
        while let Some(event) = handler.next().await {
            if event.is_err() {
                break;
            }
        }
    });

    let page = browser
        .new_page("about:blank")
        .await
        .context("Failed to open a browser tab")?;
    if let Some(ua) = user_agent {
        page.set_user_agent(ua)
            .await
            .context("Failed to set user agent")?;
    }

    Ok(BrowserSession { browser, handler_task, page, user_data_dir })
}

async fn navigate(page: &Page, url: &str, wait_seconds: u64, timeout_seconds: u64) -> Result<()> {
    let nav = async {
        page.goto(url).await?;
        page.wait_for_navigation().await?;
        Ok::<_, anyhow::Error>(())
    };
    tokio::time::timeout(Duration::from_secs(timeout_seconds), nav)
        .await
        .map_err(|_| {
            anyhow!(
                "Navigation to {} timed out after {}s. The browser session remains open; \
                 you can retry or call browser_capture_markdown to capture whatever loaded.",
                url,
                timeout_seconds
            )
        })??;
    if wait_seconds > 0 {
        tokio::time::sleep(Duration::from_secs(wait_seconds)).await;
    }
    Ok(())
}

async fn page_info(page: &Page) -> PageInfo {
    let url = page
        .url()
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "unknown".to_string());
    let title = page.get_title().await.ok().flatten();
    PageInfo { url, title }
}

/// Open (or replace) the global browser session and navigate to `url`.
/// The session stays alive after this returns so the user can interact
/// with the page before capturing it.
pub async fn open(
    url: &str,
    visible: bool,
    wait_seconds: u64,
    user_agent: Option<&str>,
    timeout_seconds: u64,
) -> Result<PageInfo> {
    let mut guard = SESSION.lock().await;
    if let Some(old) = guard.take() {
        close_session(old).await;
    }

    let session = launch(visible, user_agent).await?;
    let nav_result = navigate(&session.page, url, wait_seconds, timeout_seconds).await;
    let info = page_info(&session.page).await;
    *guard = Some(session);
    // Keep the session open even on a navigation timeout so the user can
    // intervene or capture the partial page.
    nav_result?;
    Ok(info)
}

/// Capture the rendered HTML of the current page. If `navigate_to` is given,
/// navigates there first (opening a headless session if none exists).
pub async fn capture_html(
    navigate_to: Option<&str>,
    wait_seconds: u64,
    timeout_seconds: u64,
) -> Result<(String, PageInfo)> {
    let mut guard = SESSION.lock().await;

    if guard.is_none() {
        match navigate_to {
            Some(_) => *guard = Some(launch(false, None).await?),
            None => {
                return Err(anyhow!(
                    "No browser session is open. Call browser_open_url first \
                     (or pass a url to browser_capture_markdown)."
                ))
            }
        }
    }
    let session = guard.as_ref().unwrap();

    if let Some(url) = navigate_to {
        navigate(&session.page, url, wait_seconds, timeout_seconds).await?;
    }

    let html = session.page.content().await.map_err(|e| {
        anyhow!(
            "Failed to capture page content: {}. The browser window may have been \
             closed; call browser_open_url to start a new session.",
            e
        )
    })?;
    let info = page_info(&session.page).await;
    Ok((html, info))
}

async fn close_session(mut session: BrowserSession) {
    let _ = session.browser.close().await;
    let _ = session.browser.wait().await;
    session.handler_task.abort();
    let _ = tokio::fs::remove_dir_all(&session.user_data_dir).await;
}

/// Close the global browser session if one is open. Returns true if a
/// session was closed.
pub async fn close() -> bool {
    let mut guard = SESSION.lock().await;
    match guard.take() {
        Some(session) => {
            close_session(session).await;
            true
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn capture_without_session_or_url_errors() {
        // Ensure no session is open, then verify the instructive error.
        close().await;
        let err = capture_html(None, 0, DEFAULT_NAV_TIMEOUT_SECS)
            .await
            .expect_err("expected error when no session is open");
        assert!(err.to_string().contains("browser_open_url"));
    }

    #[test]
    fn chrome_hint_mentions_env_var() {
        let err = chrome_not_found_hint("no executable found");
        assert!(err.to_string().contains("CHROME"));
        assert!(err.to_string().contains("no executable found"));
    }
}
