use std::{
    io::{Read, Write},
    net::TcpListener,
    process::{Command, Output, Stdio},
    thread,
};

use sha2::{Digest, Sha256};

const ARTICLE: &str = r#"
<html lang="ja">
<head>
  <title>魚拓: 「日本語」の記事</title>
  <meta name="author" content="山田 &quot;Yamada&quot; 太郎">
</head>
<body><article>
  <h1>魚拓: 「日本語」の記事</h1>
  <p>This English paragraph and 日本語の段落 contain enough representative article text
  for extraction without making any network request during this CLI test.</p>
  <p>二つ目の段落にも十分な本文を入れて、Markdown変換結果を安定して確認できるようにします。</p>
</article></body>
</html>
"#;

fn run_stdin(args: &[&str], input: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rdbl"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn stdin_frontmatter_wraps_unchanged_markdown_and_omits_source_url() {
    let args = ["--stdin", "--min-text", "20"];
    let plain = run_stdin(&args, ARTICLE);
    assert!(
        plain.status.success(),
        "{}",
        String::from_utf8_lossy(&plain.stderr)
    );

    let archived = run_stdin(&[&args[..], &["--frontmatter"]].concat(), ARTICLE);
    assert!(
        archived.status.success(),
        "{}",
        String::from_utf8_lossy(&archived.stderr)
    );

    let plain = String::from_utf8(plain.stdout).unwrap();
    let archived = String::from_utf8(archived.stdout).unwrap();
    let (metadata, body) = archived
        .strip_prefix("---\n")
        .unwrap()
        .split_once("---\n")
        .unwrap();

    assert_eq!(body, plain);
    assert!(metadata.contains("title: \"魚拓: 「日本語」の記事\"\n"));
    assert!(metadata.contains("byline: \"山田 \\\"Yamada\\\" 太郎\"\n"));
    assert!(!metadata.contains("source_url:"));
    assert!(metadata.contains("retrieved_at: \"20"));
    assert!(metadata.contains("generator: \"rdbl\"\n"));
    assert!(metadata.contains(&format!(
        "content_sha256: \"{:x}\"\n",
        Sha256::digest(plain.strip_suffix('\n').unwrap().as_bytes())
    )));
}

#[test]
fn frontmatter_rejects_non_markdown_format() {
    let output = run_stdin(&["--stdin", "--frontmatter", "--format", "json"], ARTICLE);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("--frontmatter and --heading-offset can only be used with --format markdown")
    );
}

#[test]
fn url_archive_embeds_images_and_resolves_reference_links() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    thread::spawn(move || {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 1024];
            let length = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..length]);
            let (content_type, body): (&str, &[u8]) = if request.starts_with("GET /image.png ") {
                ("image/png", &[1, 2, 3])
            } else {
                (
                    "text/html; charset=utf-8",
                    r#"<html><head><title>Portable archive</title></head><body><article>
                    <h1>Portable archive</h1>
                    <p>This article has enough text to verify portable Markdown output and its
                    relative references without using the public network during the test.</p>
                    <p><a href="/details">Read details</a></p>
                    <img src="/image.png" alt="Small image">
                    </article></body></html>"#
                        .as_bytes(),
                )
            };
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(body).unwrap();
        }
    });

    let output = Command::new(env!("CARGO_BIN_EXE_rdbl"))
        .args([
            "--frontmatter",
            "--min-text",
            "20",
            &format!("http://{address}/article"),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = String::from_utf8(output.stdout).unwrap();
    assert!(output.contains("[Read details][rdbl-1]"));
    assert!(output.contains("![Small image][rdbl-2]"));
    assert!(output.contains(&format!("[rdbl-1]: <http://{address}/details>")));
    assert!(output.contains("[rdbl-2]: <data:image/png;base64,AQID>"));
    assert!(!output.contains("](/details)"));
    assert!(!output.contains("](/image.png)"));
}
