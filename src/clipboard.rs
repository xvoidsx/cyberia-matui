use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use anyhow::Context;

/// Grab image data from the system clipboard, if any.
///
/// Tries Wayland (`wl-paste`) first, then X11 (`xclip`). Returns `Ok(None)`
/// when there is no image data on the clipboard or no clipboard tool is
/// available. The image bytes are written to a persisted temp file (via
/// `.keep()`) so the existing upload flow can consume the path directly.
///
/// Real failures (e.g. temp file creation) are propagated as errors.
pub fn clipboard_image() -> anyhow::Result<Option<PathBuf>> {
    // (tool, args, file extension) in preference order
    let candidates: &[(&str, &[&str], &str)] = &[
        ("wl-paste", &["--type", "image/png"], ".png"),
        ("wl-paste", &["--type", "image/jpeg"], ".jpg"),
        (
            "xclip",
            &["-selection", "clipboard", "-t", "image/png", "-o"],
            ".png",
        ),
        (
            "xclip",
            &["-selection", "clipboard", "-t", "image/jpeg", "-o"],
            ".jpg",
        ),
    ];

    for (tool, args, suffix) in candidates {
        if let Some(data) = paste_bytes(tool, args)? {
            let mut tmpfile = tempfile::Builder::new()
                .prefix("cyberia-clipboard-")
                .suffix(suffix)
                .tempfile()
                .context("could not create temp file for clipboard image")?;

            tmpfile.write_all(&data)?;

            let path = tmpfile
                .into_temp_path()
                .keep()
                .context("could not persist clipboard image temp file")?;

            return Ok(Some(path));
        }
    }

    Ok(None)
}

/// Run a clipboard tool and return its stdout, or `None` when there is no
/// usable data.
///
/// - missing binary -> `None` (feature gracefully unavailable)
/// - non-zero exit or empty stdout -> `None` (wl-paste exits 0 with empty
///   stdout when the requested MIME type isn't on the clipboard; xclip
///   exits non-zero)
fn paste_bytes(tool: &str, args: &[&str]) -> anyhow::Result<Option<Vec<u8>>> {
    let output = match Command::new(tool).args(args).output() {
        Ok(output) => output,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err.into()),
    };

    if !output.status.success() || output.stdout.is_empty() {
        return Ok(None);
    }

    Ok(Some(output.stdout))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_tool_yields_none() {
        // a tool name that cannot exist -> NotFound -> Ok(None), never an error
        let result = paste_bytes("cyberia-definitely-not-a-real-tool", &["--type", "image/png"]);
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn no_clipboard_tool_means_no_image() {
        // with PATH scrubbed of clipboard tools, clipboard_image() must
        // return Ok(None) rather than erroring
        struct PathGuard {
            old: Option<std::ffi::OsString>,
        }
        impl Drop for PathGuard {
            fn drop(&mut self) {
                // SAFETY: single-threaded test context; we restore the
                // original value on drop.
                unsafe {
                    match &self.old {
                        Some(p) => std::env::set_var("PATH", p),
                        None => std::env::remove_var("PATH"),
                    }
                }
            }
        }

        let _guard = PathGuard {
            // SAFETY: see above.
            old: unsafe {
                let old = std::env::var_os("PATH");
                std::env::set_var("PATH", "/nonexistent-dir-for-cyberia-test");
                old
            },
        };

        assert!(clipboard_image().unwrap().is_none());
    }
}
