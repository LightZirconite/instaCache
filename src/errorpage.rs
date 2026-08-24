//! Offline / load-failure page.
//!
//! Rendered in place of Chromium's default error page so a dropped connection
//! looks like part of the app instead of a browser error.

/// Shown when the page could not be loaded.
pub fn render(failing_uri: &str, message: &str) -> String {
    page(
        "Instagram is unreachable",
        message,
        "Cached pages you already visited stay available once the connection is back.",
        failing_uri,
    )
}

/// Shown when WebKit's rendering process died repeatedly and reloading it
/// automatically is no longer helping.
pub fn render_crash(failing_uri: &str, detail: &str) -> String {
    page(
        "The page stopped responding",
        detail,
        "This is usually a missing media codec or a page that ran out of memory. \
         Check the GStreamer packages listed in the README if videos never play.",
        failing_uri,
    )
}

fn page(heading: &str, message: &str, hint: &str, failing_uri: &str) -> String {
    format!(
        r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>instaCache — offline</title>
<style>
  :root {{ color-scheme: light dark; }}
  * {{ box-sizing: border-box; }}
  html, body {{ height: 100%; margin: 0; }}
  body {{
    display: grid; place-items: center;
    font: 15px/1.6 system-ui, -apple-system, "Segoe UI", Cantarell, Ubuntu, sans-serif;
    background: #fafafa; color: #262626;
    padding: 2rem;
  }}
  main {{ max-width: 30rem; text-align: center; }}
  .mark {{
    width: 68px; height: 68px; margin: 0 auto 1.5rem;
    border-radius: 20px; display: grid; place-items: center;
    background: linear-gradient(135deg, #f9ce34 0%, #ee2a7b 50%, #6228d7 100%);
  }}
  .mark svg {{ width: 36px; height: 36px; }}
  h1 {{ font-size: 1.35rem; margin: 0 0 .6rem; letter-spacing: -.01em; }}
  p {{ margin: 0 0 .5rem; color: #737373; }}
  code {{
    display: block; margin: 1.25rem 0 1.75rem; padding: .7rem .9rem;
    font-size: .8rem; word-break: break-all; text-align: left;
    background: #efefef; border-radius: 10px; color: #555;
  }}
  button {{
    font: inherit; font-weight: 600; color: #fff; cursor: pointer;
    border: 0; border-radius: 10px; padding: .7rem 1.6rem;
    background: #0095f6;
  }}
  button:hover {{ background: #1877f2; }}
  button:active {{ transform: translateY(1px); }}
  @media (prefers-color-scheme: dark) {{
    body {{ background: #000; color: #f5f5f5; }}
    p {{ color: #a8a8a8; }}
    code {{ background: #1a1a1a; color: #b0b0b0; }}
  }}
</style>
</head>
<body>
  <main>
    <div class="mark" aria-hidden="true">
      <svg viewBox="0 0 24 24" fill="none" stroke="#fff" stroke-width="2"
           stroke-linecap="round" stroke-linejoin="round">
        <path d="M1 1l22 22"/><path d="M16.7 16.7A10.9 10.9 0 0 1 12 18"/>
        <path d="M5 12.5a10 10 0 0 1 5.2-2.4"/><path d="M2 8.8a16 16 0 0 1 4.5-2.7"/>
        <path d="M22 8.8a16 16 0 0 0-8.7-3.7"/><path d="M12 21h.01"/>
      </svg>
    </div>
    <h1>{heading}</h1>
    <p>{message}</p>
    <p>{hint}</p>
    <code>{uri}</code>
    <button onclick="location.reload()" autofocus>Try again</button>
  </main>
</body>
</html>"##,
        heading = escape(heading),
        message = escape(message),
        hint = escape(hint),
        uri = escape(failing_uri),
    )
}

/// The failing URI and the GLib error string are attacker-influenced (a page
/// can navigate anywhere), so both are escaped before being embedded.
fn escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_injected_markup() {
        let html = render("https://x/?a=<script>alert(1)</script>", "boom & bust");
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("boom &amp; bust"));
    }
}
