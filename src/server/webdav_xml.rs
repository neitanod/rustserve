use std::time::SystemTime;

use chrono::{DateTime, Utc};

pub struct DavEntry {
    pub href: String,
    pub display_name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub content_type: String,
}

pub fn multistatus(entries: &[DavEntry]) -> String {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<D:multistatus xmlns:D=\"DAV:\">\n");
    for entry in entries {
        write_response(&mut xml, entry);
    }
    xml.push_str("</D:multistatus>\n");
    xml
}

fn write_response(xml: &mut String, entry: &DavEntry) {
    xml.push_str("<D:response>\n");
    xml.push_str(&format!("<D:href>{}</D:href>\n", xml_escape(&entry.href)));
    xml.push_str("<D:propstat>\n<D:prop>\n");
    xml.push_str(&format!(
        "<D:displayname>{}</D:displayname>\n",
        xml_escape(&entry.display_name)
    ));

    if entry.is_dir {
        xml.push_str("<D:resourcetype><D:collection/></D:resourcetype>\n");
    } else {
        xml.push_str("<D:resourcetype/>\n");
        xml.push_str(&format!(
            "<D:getcontentlength>{}</D:getcontentlength>\n",
            entry.size
        ));
        xml.push_str(&format!(
            "<D:getcontenttype>{}</D:getcontenttype>\n",
            xml_escape(&entry.content_type)
        ));
    }

    if let Some(t) = entry.modified {
        let dt: DateTime<Utc> = t.into();
        xml.push_str(&format!(
            "<D:getlastmodified>{}</D:getlastmodified>\n",
            dt.format("%a, %d %b %Y %H:%M:%S GMT")
        ));
        xml.push_str(&format!(
            "<D:creationdate>{}</D:creationdate>\n",
            dt.format("%Y-%m-%dT%H:%M:%SZ")
        ));
    }

    xml.push_str("</D:prop>\n");
    xml.push_str("<D:status>HTTP/1.1 200 OK</D:status>\n");
    xml.push_str("</D:propstat>\n");
    xml.push_str("</D:response>\n");
}

pub fn lock_discovery(token: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<D:prop xmlns:D="DAV:">
<D:lockdiscovery>
<D:activelock>
<D:locktype><D:write/></D:locktype>
<D:lockscope><D:exclusive/></D:lockscope>
<D:depth>0</D:depth>
<D:timeout>Second-3600</D:timeout>
<D:locktoken><D:href>urn:uuid:{token}</D:href></D:locktoken>
</D:activelock>
</D:lockdiscovery>
</D:prop>
"#
    )
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multistatus_file() {
        let entries = vec![DavEntry {
            href: "/test.txt".into(),
            display_name: "test.txt".into(),
            is_dir: false,
            size: 100,
            modified: None,
            content_type: "text/plain".into(),
        }];
        let xml = multistatus(&entries);
        assert!(xml.contains("<D:multistatus"));
        assert!(xml.contains("<D:href>/test.txt</D:href>"));
        assert!(xml.contains("<D:resourcetype/>"));
        assert!(xml.contains("<D:getcontentlength>100</D:getcontentlength>"));
    }

    #[test]
    fn test_multistatus_directory() {
        let entries = vec![DavEntry {
            href: "/mydir/".into(),
            display_name: "mydir".into(),
            is_dir: true,
            size: 0,
            modified: None,
            content_type: "httpd/unix-directory".into(),
        }];
        let xml = multistatus(&entries);
        assert!(xml.contains("<D:collection/>"));
        assert!(!xml.contains("<D:getcontentlength>"));
    }

    #[test]
    fn test_lock_discovery() {
        let xml = lock_discovery("test-uuid-123");
        assert!(xml.contains("urn:uuid:test-uuid-123"));
        assert!(xml.contains("<D:write/>"));
    }

    #[test]
    fn test_xml_escape() {
        assert_eq!(xml_escape("a&b<c>d\"e"), "a&amp;b&lt;c&gt;d&quot;e");
    }
}
