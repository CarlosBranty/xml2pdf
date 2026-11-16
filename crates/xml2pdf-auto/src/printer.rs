//! Printing through the operating system.
//!
//! Linux and macOS print with CUPS (`lp`). Windows has no command that prints
//! a PDF, so SumatraPDF is used when installed, falling back to the `PrintTo`
//! shell verb of whichever PDF application is registered.

use std::path::{Path, PathBuf};
use std::process::Command;

use xml2pdf_render::PaperFormat;

#[derive(Debug, thiserror::Error)]
pub enum PrintError {
    #[error("cannot run `{program}`: {source}. {hint}")]
    Spawn {
        program: String,
        source: std::io::Error,
        hint: &'static str,
    },
    #[error("`{program}` failed: {message}")]
    Failed { program: String, message: String },
}

/// A PDF to print.
#[derive(Debug, Clone, PartialEq)]
pub struct PrintJob<'a> {
    pub pdf: &'a Path,
    /// Printer name. `None` uses the system default printer.
    pub printer: Option<&'a str>,
    pub copies: u32,
    /// CUPS media name, see [`media_for`].
    pub media: Option<String>,
}

/// Port to the system print service, replaceable in tests.
pub trait Printer: Send + Sync {
    /// Names of the installed printers.
    fn list(&self) -> Result<Vec<String>, PrintError>;
    fn print(&self, job: &PrintJob) -> Result<(), PrintError>;
}

/// CUPS media for a format. Tickets use a custom size matching the rendered
/// page, so the printer does not feed a full default page.
pub fn media_for(format: PaperFormat, page_mm: (f64, f64)) -> Option<String> {
    match format {
        PaperFormat::A4 => Some("A4".to_owned()),
        PaperFormat::A5 => Some("A5".to_owned()),
        PaperFormat::Ticket80 | PaperFormat::Ticket58 if page_mm.1 > 0.0 => Some(format!(
            "Custom.{}x{}mm",
            format.width_mm(),
            page_mm.1.ceil()
        )),
        _ => None,
    }
}

/// Prints with the tools of the operating system.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemPrinter;

impl Printer for SystemPrinter {
    fn list(&self) -> Result<Vec<String>, PrintError> {
        if cfg!(windows) {
            let output = run(
                "powershell",
                &powershell(LIST_PRINTERS_SCRIPT),
                WINDOWS_HINT,
            )?;
            Ok(lines(&output))
        } else {
            let output = run("lpstat", &["-e".to_owned()], CUPS_HINT)?;
            Ok(lines(&output))
        }
    }

    fn print(&self, job: &PrintJob) -> Result<(), PrintError> {
        if !cfg!(windows) {
            return run("lp", &lp_arguments(job), CUPS_HINT).map(drop);
        }
        match find_sumatra() {
            Some(sumatra) => run(
                &sumatra.to_string_lossy(),
                &sumatra_arguments(job),
                WINDOWS_HINT,
            )
            .map(drop),
            None => {
                // The shell verb prints one copy per call.
                (0..job.copies.max(1)).try_for_each(|_| {
                    run(
                        "powershell",
                        &powershell(&print_to_script(job)),
                        WINDOWS_HINT,
                    )
                    .map(drop)
                })
            }
        }
    }
}

const CUPS_HINT: &str = "Install CUPS to print on Linux or macOS";
const WINDOWS_HINT: &str = "Install SumatraPDF for reliable silent printing on Windows";
const LIST_PRINTERS_SCRIPT: &str = "Get-CimInstance Win32_Printer | ForEach-Object { $_.Name }";

/// Arguments of the CUPS `lp` command for a job.
pub(crate) fn lp_arguments(job: &PrintJob) -> Vec<String> {
    let mut arguments = Vec::new();
    if let Some(printer) = job.printer {
        arguments.extend(["-d".to_owned(), printer.to_owned()]);
    }
    arguments.extend(["-n".to_owned(), job.copies.max(1).to_string()]);
    if let Some(media) = &job.media {
        arguments.extend(["-o".to_owned(), format!("media={media}")]);
    }
    arguments.extend(["--".to_owned(), job.pdf.to_string_lossy().into_owned()]);
    arguments
}

/// Arguments of SumatraPDF for a silent print.
pub(crate) fn sumatra_arguments(job: &PrintJob) -> Vec<String> {
    let mut arguments = match job.printer {
        Some(printer) => vec!["-print-to".to_owned(), printer.to_owned()],
        None => vec!["-print-to-default".to_owned()],
    };
    arguments.extend([
        "-print-settings".to_owned(),
        format!("{}x,noscale", job.copies.max(1)),
        "-silent".to_owned(),
        job.pdf.to_string_lossy().into_owned(),
    ]);
    arguments
}

/// PowerShell script printing through the registered PDF application.
pub(crate) fn print_to_script(job: &PrintJob) -> String {
    let file = powershell_quote(&job.pdf.to_string_lossy());
    match job.printer {
        Some(printer) => format!(
            "Start-Process -FilePath {file} -Verb PrintTo -ArgumentList {} -WindowStyle Hidden -Wait",
            powershell_quote(&format!("\"{printer}\""))
        ),
        None => format!("Start-Process -FilePath {file} -Verb Print -WindowStyle Hidden -Wait"),
    }
}

/// Quotes a value as a PowerShell single-quoted string.
fn powershell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn powershell(script: &str) -> Vec<String> {
    ["-NoProfile", "-NonInteractive", "-Command", script]
        .map(str::to_owned)
        .to_vec()
}

/// Looks for SumatraPDF: `XML2PDF_SUMATRA`, next to the executable, then the
/// usual installation folders.
fn find_sumatra() -> Option<PathBuf> {
    const EXECUTABLE: &str = "SumatraPDF.exe";
    let beside_executable = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|folder| folder.join(EXECUTABLE)));
    let installed = ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(|folder| PathBuf::from(folder).join("SumatraPDF").join(EXECUTABLE));

    std::env::var_os("XML2PDF_SUMATRA")
        .map(PathBuf::from)
        .into_iter()
        .chain(beside_executable)
        .chain(installed)
        .find(|candidate| candidate.is_file())
}

fn lines(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

fn run(program: &str, arguments: &[String], hint: &'static str) -> Result<String, PrintError> {
    let output = Command::new(program)
        .args(arguments)
        .output()
        .map_err(|source| PrintError::Spawn {
            program: program.to_owned(),
            source,
            hint,
        })?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(PrintError::Failed {
            program: program.to_owned(),
            message,
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job<'a>(printer: Option<&'a str>, copies: u32, media: Option<&str>) -> PrintJob<'a> {
        PrintJob {
            pdf: Path::new("/tmp/F001-1.pdf"),
            printer,
            copies,
            media: media.map(str::to_owned),
        }
    }

    #[test]
    fn builds_the_lp_command() {
        assert_eq!(
            lp_arguments(&job(None, 1, None)),
            ["-n", "1", "--", "/tmp/F001-1.pdf"]
        );
        assert_eq!(
            lp_arguments(&job(Some("Caja 1"), 3, Some("Custom.80x164mm"))),
            [
                "-d",
                "Caja 1",
                "-n",
                "3",
                "-o",
                "media=Custom.80x164mm",
                "--",
                "/tmp/F001-1.pdf"
            ]
        );
        assert_eq!(
            lp_arguments(&job(None, 0, None))[1],
            "1",
            "at least one copy is printed"
        );
    }

    #[test]
    fn builds_the_sumatra_command() {
        assert_eq!(
            sumatra_arguments(&job(None, 1, None)),
            [
                "-print-to-default",
                "-print-settings",
                "1x,noscale",
                "-silent",
                "/tmp/F001-1.pdf"
            ]
        );
        assert_eq!(
            sumatra_arguments(&job(Some("Caja 1"), 2, None))[..4],
            ["-print-to", "Caja 1", "-print-settings", "2x,noscale"]
        );
    }

    #[test]
    fn quotes_powershell_arguments() {
        let named = job(Some("Caja d'Or"), 1, None);
        assert_eq!(
            print_to_script(&named),
            "Start-Process -FilePath '/tmp/F001-1.pdf' -Verb PrintTo -ArgumentList '\"Caja d''Or\"' -WindowStyle Hidden -Wait"
        );
        assert_eq!(
            print_to_script(&job(None, 1, None)),
            "Start-Process -FilePath '/tmp/F001-1.pdf' -Verb Print -WindowStyle Hidden -Wait"
        );
    }

    #[test]
    fn splits_printer_lists_into_names() {
        assert_eq!(lines("Caja\n  Oficina HP \n\n"), ["Caja", "Oficina HP"]);
    }
}
