//! End-to-end behavior of the `xml2pdf` binary.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

fn xml2pdf() -> Command {
    Command::new(env!("CARGO_BIN_EXE_xml2pdf"))
}

fn run(command: &mut Command) -> (i32, String, String) {
    let Output {
        status,
        stdout,
        stderr,
    } = command.output().unwrap();
    (
        status.code().unwrap_or(-1),
        String::from_utf8_lossy(&stdout).into_owned(),
        String::from_utf8_lossy(&stderr).into_owned(),
    )
}

fn is_pdf(path: &Path) -> bool {
    std::fs::read(path).is_ok_and(|bytes| bytes.starts_with(b"%PDF-"))
}

fn file_names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn converts_a_file_to_the_requested_output() {
    let folder = tempfile::tempdir().unwrap();
    let output = folder.path().join("salida.pdf");

    let (code, stdout, stderr) = run(xml2pdf()
        .arg("convert")
        .arg(fixture("factura.xml"))
        .arg("-o")
        .arg(&output));

    assert_eq!((code, stderr.as_str()), (0, ""));
    assert_eq!(stdout.trim(), output.to_string_lossy());
    assert!(is_pdf(&output));
}

#[test]
fn writes_the_pdf_next_to_the_input_by_default() {
    let folder = tempfile::tempdir().unwrap();
    let input = folder.path().join("F001-123.xml");
    std::fs::copy(fixture("factura.xml"), &input).unwrap();

    let (code, _, _) = run(xml2pdf().arg("convert").arg(&input));

    assert_eq!(code, 0);
    assert!(is_pdf(&folder.path().join("F001-123.pdf")));
}

#[test]
fn converts_every_document_of_a_folder() {
    let output = tempfile::tempdir().unwrap();

    let (code, stdout, _) = run(xml2pdf()
        .arg("convert")
        .arg(fixture(""))
        .args(["--format", "ticket80", "-o"])
        .arg(output.path()));

    assert_eq!(code, 0);
    assert_eq!(stdout.lines().count(), 6);
    assert_eq!(
        file_names(output.path()),
        [
            // Documents inside the ZIP are named after their identifier.
            "20123456789-01-F001-00000123.pdf",
            "20123456789-07-FC01-00000008.pdf",
            "boleta.pdf",
            "factura.pdf",
            "nota-credito.pdf",
            "nota-debito.pdf",
        ]
    );
    assert!(
        file_names(output.path())
            .iter()
            .all(|name| is_pdf(&output.path().join(name)))
    );
}

#[test]
fn keeps_converting_after_a_failure_and_exits_with_an_error() {
    let folder = tempfile::tempdir().unwrap();
    let broken = folder.path().join("roto.xml");
    std::fs::write(&broken, "<ApplicationResponse/>").unwrap();
    let output = folder.path().join("pdf");

    let (code, stdout, stderr) = run(xml2pdf()
        .arg("convert")
        .arg(&broken)
        .arg(fixture("boleta.xml"))
        .arg("-o")
        .arg(&output));

    assert_eq!(code, 1);
    assert!(
        stderr.contains("roto.xml: unsupported document `ApplicationResponse`"),
        "{stderr}"
    );
    assert!(stderr.contains("error: 1 of 2 files failed"), "{stderr}");
    assert_eq!(stdout.lines().count(), 1);
    assert_eq!(file_names(&output), ["boleta.pdf"]);
}

#[test]
fn rejects_a_single_output_file_for_several_documents() {
    let folder = tempfile::tempdir().unwrap();
    let output = folder.path().join("todo.pdf");

    let (code, _, stderr) = run(xml2pdf()
        .arg("convert")
        .arg(fixture("factura.xml"))
        .arg(fixture("boleta.xml"))
        .arg("-o")
        .arg(&output));
    assert_eq!(code, 1);
    assert!(
        stderr.contains("`--output` must be a directory when converting several files"),
        "{stderr}"
    );

    let (code, _, stderr) = run(xml2pdf()
        .arg("convert")
        .arg(fixture("documentos.zip"))
        .arg("-o")
        .arg(&output));
    assert_eq!(code, 1);
    assert!(stderr.contains("contains 2 documents"), "{stderr}");
    assert!(!output.exists());
}

#[test]
fn rejects_invalid_style_options() {
    let folder = tempfile::tempdir().unwrap();
    let output = folder.path().join("salida.pdf");
    let convert = |extra: &[&str]| {
        run(xml2pdf()
            .arg("convert")
            .arg(fixture("factura.xml"))
            .arg("-o")
            .arg(&output)
            .args(extra))
    };

    let (code, _, stderr) = convert(&["--format", "letter"]);
    assert_eq!(code, 2);
    assert!(stderr.contains("unknown format `letter`"), "{stderr}");

    let (code, _, stderr) = convert(&["--accent", "#aa0000"]);
    assert_eq!(code, 2, "documents have no color option");
    assert!(
        stderr.contains("unexpected argument '--accent'"),
        "{stderr}"
    );

    let (code, _, stderr) = convert(&["--logo", "/does/not/exist.png"]);
    assert_eq!(code, 1);
    assert!(
        stderr.contains("cannot read `/does/not/exist.png`"),
        "{stderr}"
    );
    assert!(
        !output.exists(),
        "nothing is written when the options are invalid"
    );
}

#[test]
fn applies_a_configuration_file_and_lets_options_override_it() {
    let folder = tempfile::tempdir().unwrap();
    let config = folder.path().join("config.json");
    std::fs::write(&config, r##"{"format":"ticket58","footer":"Gracias"}"##).unwrap();

    let data = |extra: &[&str]| {
        let (code, stdout, stderr) = run(xml2pdf()
            .arg("data")
            .arg(fixture("factura.xml"))
            .arg("--config")
            .arg(&config)
            .args(extra));
        assert_eq!(code, 0, "{stderr}");
        stdout
    };

    let from_config = data(&[]);
    assert!(
        from_config.contains(r#""format": "ticket58""#),
        "{from_config}"
    );
    assert!(from_config.contains(r#""footer": "Gracias""#));
    assert!(from_config.contains(r#""qr": "qr.svg""#));

    let overridden = data(&["--format", "a5", "--footer", "Hasta pronto", "--no-qr"]);
    assert!(overridden.contains(r#""format": "a5""#), "{overridden}");
    assert!(overridden.contains(r#""footer": "Hasta pronto""#));
    assert!(overridden.contains(r#""qr": null"#));
    assert!(
        overridden.contains(r#""issuer""#),
        "the document data is still printed"
    );

    std::fs::write(&config, r#"{"paper":"a4"}"#).unwrap();
    let (code, _, stderr) = run(xml2pdf()
        .arg("data")
        .arg(fixture("factura.xml"))
        .arg("--config")
        .arg(&config));
    assert_eq!(code, 1);
    assert!(stderr.contains("unknown field `paper`"), "{stderr}");
}

#[test]
fn renders_with_a_custom_template() {
    let folder = tempfile::tempdir().unwrap();
    let output = folder.path().join("salida.pdf");
    let template = folder.path().join("mine.typ");

    let (code, builtin, _) = run(xml2pdf().args(["template", "--format", "ticket80"]));
    assert_eq!(code, 0);
    assert!(builtin.contains("json(\"data.json\")"));
    std::fs::write(&template, &builtin).unwrap();

    let (code, _, stderr) = run(xml2pdf()
        .arg("convert")
        .arg(fixture("boleta.xml"))
        .arg("--template")
        .arg(&template)
        .arg("-o")
        .arg(&output));
    assert_eq!(code, 0, "{stderr}");
    assert!(is_pdf(&output));

    std::fs::write(&template, "#let d = json(\"data.json\")\n#d.nope").unwrap();
    let broken_output = folder.path().join("rota.pdf");
    let (code, _, stderr) = run(xml2pdf()
        .arg("convert")
        .arg(fixture("boleta.xml"))
        .arg("--template")
        .arg(&template)
        .arg("-o")
        .arg(&broken_output));
    assert_eq!(code, 1);
    assert!(
        stderr.contains("the template failed to compile"),
        "{stderr}"
    );
    assert!(stderr.contains("line 2:"), "{stderr}");
    assert!(!broken_output.exists());
}

#[test]
fn watch_requires_print_for_printer_options() {
    let folder = tempfile::tempdir().unwrap();

    let (code, _, stderr) = run(xml2pdf()
        .arg("watch")
        .arg(folder.path())
        .args(["--printer", "Caja"]));
    assert_eq!(code, 2);
    assert!(stderr.contains("--print"), "{stderr}");

    let (code, _, stderr) = run(xml2pdf()
        .arg("watch")
        .arg(folder.path())
        .args(["--print", "--copies", "0"]));
    assert_eq!(code, 2);
    assert!(stderr.contains("--copies"), "{stderr}");

    let (code, _, stderr) = run(xml2pdf().arg("watch").arg(folder.path().join("missing")));
    assert_eq!(code, 1);
    assert!(stderr.contains("does not exist"), "{stderr}");
}

/// Runs `xml2pdf watch --print` against a stand-in `lp` that records its
/// arguments, covering the real path from a new file to the print command.
#[cfg(unix)]
#[test]
fn watch_prints_new_documents_with_lp() {
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};

    let folder = tempfile::tempdir().unwrap();
    let watched = folder.path().join("descargas");
    let bin = folder.path().join("bin");
    let record = folder.path().join("lp-arguments.txt");
    std::fs::create_dir_all(&watched).unwrap();
    std::fs::create_dir_all(&bin).unwrap();
    let fake_lp = bin.join("lp");
    std::fs::write(
        &fake_lp,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\n",
            record.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&fake_lp, std::fs::Permissions::from_mode(0o755)).unwrap();

    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut child = xml2pdf()
        .arg("watch")
        .arg(&watched)
        .args([
            "--print",
            "--printer",
            "Caja",
            "--copies",
            "2",
            "--format",
            "a5",
        ])
        .env("PATH", path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();

    // Give the watcher time to start, then drop a document into the folder.
    std::thread::sleep(Duration::from_millis(800));
    std::fs::copy(fixture("factura.xml"), watched.join("F001-123.xml")).unwrap();

    let deadline = Instant::now() + Duration::from_secs(20);
    while !record.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    child.kill().unwrap();
    child.wait().unwrap();

    let pdf = watched.join("pdf/20123456789-01-F001-00000123.pdf");
    assert!(is_pdf(&pdf), "the PDF was not created");
    let arguments = std::fs::read_to_string(&record).expect("lp was not called");
    assert_eq!(
        arguments.lines().collect::<Vec<_>>(),
        [
            "-d",
            "Caja",
            "-n",
            "2",
            "-o",
            "media=A5",
            "--",
            pdf.to_str().unwrap()
        ]
    );
}
