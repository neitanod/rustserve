use std::path::Path;
use std::time::SystemTime;
use chrono::{DateTime, Local};
use humansize::{format_size, BINARY};

pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub mime_type: String,
}

pub fn scan_directory(path: &Path) -> std::io::Result<Vec<FileEntry>> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = meta.is_dir();
        let mime_type = if is_dir {
            "directory".into()
        } else {
            mime_guess::from_path(&name)
                .first_or_octet_stream()
                .to_string()
        };
        entries.push(FileEntry {
            name,
            is_dir,
            size: if is_dir { 0 } else { meta.len() },
            modified: meta.modified().ok(),
            mime_type,
        });
    }
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

pub fn render_listing(request_path: &str, entries: &[FileEntry], upload_enabled: bool) -> String {
    let breadcrumbs = build_breadcrumbs(request_path);
    let display_path = percent_encoding::percent_decode_str(request_path)
        .decode_utf8()
        .unwrap_or(std::borrow::Cow::Borrowed(request_path));
    let rows: String = entries
        .iter()
        .map(|e| {
            let icon = file_icon(e);
            let href = if request_path == "/" || request_path.is_empty() {
                format!(
                    "/{}",
                    percent_encoding::utf8_percent_encode(
                        &e.name,
                        percent_encoding::NON_ALPHANUMERIC
                    )
                )
            } else {
                let trimmed = request_path.trim_end_matches('/');
                format!(
                    "{}/{}",
                    trimmed,
                    percent_encoding::utf8_percent_encode(
                        &e.name,
                        percent_encoding::NON_ALPHANUMERIC
                    )
                )
            };
            let name_display = if e.is_dir {
                format!("{}/", e.name)
            } else {
                e.name.clone()
            };
            let size_display = if e.is_dir {
                "\u{2014}".to_string()
            } else {
                format_size(e.size, BINARY)
            };
            let date_display = e
                .modified
                .map(|t| {
                    let dt: DateTime<Local> = t.into();
                    dt.format("%Y-%m-%d %H:%M").to_string()
                })
                .unwrap_or_else(|| "\u{2014}".into());

            format!(
                r#"<tr onclick="window.location='{href}'">
<td class="icon">{icon}</td>
<td class="name"><a href="{href}">{name_display}</a></td>
<td class="size">{size_display}</td>
<td class="date">{date_display}</td>
<td class="mime">{mime}</td>
</tr>"#,
                mime = e.mime_type
            )
        })
        .collect();

    let upload_form = if upload_enabled {
        r#"<div class="upload-area" id="upload-area">
<form method="POST" enctype="multipart/form-data" id="upload-form">
<input type="file" name="file" id="file-input" multiple>
<button type="submit" id="upload-btn">Upload</button>
</form>
<div class="progress-wrap" id="progress-wrap">
<div class="progress-bar" id="progress-bar"></div>
<span class="progress-text" id="progress-text">0%</span>
</div>
</div>
<script>
(function(){
  var form=document.getElementById('upload-form'),
      wrap=document.getElementById('progress-wrap'),
      bar=document.getElementById('progress-bar'),
      txt=document.getElementById('progress-text'),
      btn=document.getElementById('upload-btn');
  form.addEventListener('submit',function(e){
    e.preventDefault();
    var fd=new FormData(form);
    var xhr=new XMLHttpRequest();
    wrap.style.display='flex';
    btn.disabled=true;
    btn.textContent='Uploading...';
    xhr.upload.addEventListener('progress',function(ev){
      if(ev.lengthComputable){
        var pct=Math.round(ev.loaded/ev.total*100);
        bar.style.width=pct+'%';
        txt.textContent=pct+'% ('+fmt(ev.loaded)+' / '+fmt(ev.total)+')';
      }
    });
    xhr.addEventListener('load',function(){
      bar.style.width='100%';
      txt.textContent='Done!';
      btn.textContent='Upload';
      btn.disabled=false;
      setTimeout(function(){location.reload();},600);
    });
    xhr.addEventListener('error',function(){
      txt.textContent='Upload failed';
      bar.style.background='#da3633';
      btn.textContent='Upload';
      btn.disabled=false;
    });
    xhr.open('POST',location.pathname);
    xhr.send(fd);
  });
  function fmt(b){
    if(b<1024)return b+'B';
    if(b<1048576)return(b/1024).toFixed(1)+'KB';
    if(b<1073741824)return(b/1048576).toFixed(1)+'MB';
    return(b/1073741824).toFixed(2)+'GB';
  }
})();
</script>"#
    } else {
        ""
    };

    let parent_row = if request_path != "/" && !request_path.is_empty() {
        let parent = request_path.trim_end_matches('/');
        let parent = parent.rsplit_once('/').map(|(p, _)| p).unwrap_or("/");
        let parent = if parent.is_empty() { "/" } else { parent };
        format!(
            r#"<tr onclick="window.location='{parent}'">
<td class="icon">&#x1F519;</td>
<td class="name"><a href="{parent}">..</a></td>
<td class="size">&mdash;</td>
<td class="date">&mdash;</td>
<td class="mime">&mdash;</td>
</tr>"#
        )
    } else {
        String::new()
    };

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>serve &mdash; {path}</title>
<style>
* {{ margin: 0; padding: 0; box-sizing: border-box; }}
body {{
    background: #0d1117;
    color: #c9d1d9;
    font-family: 'JetBrains Mono', 'Fira Code', 'Monaco', monospace;
    min-height: 100vh;
    padding: 1.5rem;
    line-height: 1.6;
}}
.container {{ max-width: 960px; margin: 0 auto; }}
.header {{
    margin-bottom: 1.5rem;
    padding-bottom: 1rem;
    border-bottom: 1px solid #30363d;
}}
.logo {{ font-size: 1.3rem; font-weight: 700; color: #58a6ff; }}
.logo .accent {{ color: #f0883e; }}
.breadcrumbs {{ margin-top: 0.5rem; font-size: 0.85rem; color: #8b949e; }}
.breadcrumbs a {{ color: #58a6ff; text-decoration: none; }}
.breadcrumbs a:hover {{ text-decoration: underline; }}
.breadcrumbs .sep {{ color: #6e7681; margin: 0 0.3rem; }}
table {{
    width: 100%;
    border-collapse: collapse;
    margin-top: 1rem;
    font-size: 0.85rem;
}}
th {{
    text-align: left;
    padding: 0.6rem 0.8rem;
    border-bottom: 2px solid #30363d;
    color: #f0883e;
    font-weight: 400;
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
}}
td {{
    padding: 0.45rem 0.8rem;
    border-bottom: 1px solid #21262d;
}}
tr:hover {{ background: #161b22; cursor: pointer; }}
.icon {{ width: 2rem; text-align: center; }}
.name a {{ color: #58a6ff; text-decoration: none; }}
.name a:hover {{ text-decoration: underline; }}
.size {{ color: #8b949e; text-align: right; width: 7rem; }}
.date {{ color: #8b949e; width: 10rem; }}
.mime {{ color: #6e7681; font-size: 0.75rem; }}
.upload-area {{
    margin-top: 1.5rem;
    padding: 1rem;
    border: 1px dashed #30363d;
    border-radius: 6px;
    background: #161b22;
}}
.upload-area button {{
    background: #238636;
    color: #fff;
    border: none;
    padding: 0.4rem 1rem;
    border-radius: 4px;
    cursor: pointer;
    font-family: inherit;
    margin-left: 0.5rem;
}}
.upload-area button:hover {{ background: #2ea043; }}
.upload-area input[type=file] {{ color: #c9d1d9; }}
.upload-area button:disabled {{ opacity: 0.6; cursor: default; }}
.progress-wrap {{
    display: none;
    align-items: center;
    margin-top: 0.8rem;
    gap: 0.8rem;
    height: 1.4rem;
}}
.progress-bar {{
    flex: 1;
    height: 100%;
    background: #238636;
    border-radius: 4px;
    width: 0%;
    transition: width 0.2s;
}}
.progress-wrap {{
    background: #21262d;
    border-radius: 4px;
    overflow: hidden;
    position: relative;
}}
.progress-text {{
    position: absolute;
    right: 0.5rem;
    top: 0;
    line-height: 1.4rem;
    font-size: 0.75rem;
    color: #c9d1d9;
    white-space: nowrap;
}}
.footer {{
    margin-top: 2rem;
    padding-top: 0.8rem;
    border-top: 1px solid #30363d;
    color: #6e7681;
    font-size: 0.7rem;
    text-align: center;
}}
@media (max-width: 700px) {{
    .date, .mime {{ display: none; }}
    body {{ padding: 0.8rem; }}
}}
</style>
</head>
<body>
<div class="container">
<div class="header">
<div class="logo"><span class="accent">serve</span> file server</div>
<div class="breadcrumbs">{breadcrumbs}</div>
</div>
<table>
<thead>
<tr><th></th><th>Name</th><th class="size">Size</th><th>Modified</th><th>Type</th></tr>
</thead>
<tbody>
{parent_row}
{rows}
</tbody>
</table>
{upload_form}
<div class="footer">serve &mdash; rust file server</div>
</div>
</body>
</html>"##,
        path = display_path,
    )
}

fn build_breadcrumbs(path: &str) -> String {
    let mut crumbs = vec![r#"<a href="/">/</a>"#.to_string()];
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let mut accumulated = String::new();
    for part in &parts {
        accumulated.push('/');
        accumulated.push_str(part);
        let display = percent_encoding::percent_decode_str(part)
            .decode_utf8()
            .unwrap_or(std::borrow::Cow::Borrowed(part));
        crumbs.push(format!(
            r#"<span class="sep">/</span><a href="{accumulated}">{display}</a>"#
        ));
    }
    crumbs.join("")
}

fn file_icon(entry: &FileEntry) -> &'static str {
    if entry.is_dir {
        "\u{1F4C1}"
    } else if entry.mime_type.starts_with("image/") {
        "\u{1F5BC}"
    } else if entry.mime_type.starts_with("video/") {
        "\u{1F3AC}"
    } else if entry.mime_type.starts_with("audio/") {
        "\u{1F3B5}"
    } else if entry.mime_type.starts_with("text/")
        || entry.mime_type.contains("json")
        || entry.mime_type.contains("xml")
        || entry.mime_type.contains("javascript")
    {
        "\u{1F4C4}"
    } else if entry.mime_type.contains("zip")
        || entry.mime_type.contains("tar")
        || entry.mime_type.contains("gzip")
        || entry.mime_type.contains("rar")
    {
        "\u{1F4E6}"
    } else if entry.mime_type.contains("pdf") {
        "\u{1F4D5}"
    } else {
        "\u{1F4CE}"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_scan_directory_sorted() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("b.txt"), "b").unwrap();
        fs::write(dir.path().join("a.txt"), "a").unwrap();
        fs::create_dir(dir.path().join("zdir")).unwrap();

        let entries = scan_directory(dir.path()).unwrap();
        assert!(entries[0].is_dir);
        assert_eq!(entries[0].name, "zdir");
        assert_eq!(entries[1].name, "a.txt");
        assert_eq!(entries[2].name, "b.txt");
    }

    #[test]
    fn test_render_listing_contains_filenames() {
        let entries = vec![FileEntry {
            name: "test.txt".into(),
            is_dir: false,
            size: 1234,
            modified: None,
            mime_type: "text/plain".into(),
        }];
        let html = render_listing("/", &entries, false);
        assert!(html.contains("test.txt"));
        assert!(html.contains("#0d1117"));
    }

    #[test]
    fn test_breadcrumbs() {
        let bc = build_breadcrumbs("/foo/bar");
        assert!(bc.contains(r#"href="/foo""#));
        assert!(bc.contains(r#"href="/foo/bar""#));
    }
}
