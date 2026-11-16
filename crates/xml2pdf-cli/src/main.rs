//! `xml2pdf`: converts SUNAT electronic documents (UBL 2.1 XML) to PDF.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use clap::{Args, Parser, Subcommand};
use xml2pdf_auto::{AutoConfig, Printer, SystemPrinter, WatchEvent};
use xml2pdf_render::{PaperFormat, RenderOptions, config};

#[derive(Parser)]
#[command(
    name = "xml2pdf",
    version,
    about = "Convert SUNAT XML documents (UBL 2.1) to printable PDFs"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Convert XML or ZIP files, or every such file in a directory, to PDF
    Convert(ConvertArgs),
    /// Print the built-in Typst template of a format, to start a custom one
    Template {
        /// Paper format: a4, a5, ticket80 or ticket58
        #[arg(short, long, default_value = "a4")]
        format: PaperFormat,
    },
    /// Print the data a template receives for a document, as JSON
    Data {
        /// XML or ZIP file
        input: PathBuf,
        #[command(flatten)]
        style: StyleArgs,
    },
    /// Watch a folder and convert, and optionally print, every new XML or ZIP
    Watch(WatchArgs),
    /// List the printers available on this computer
    Printers,
}

#[derive(Args)]
struct WatchArgs {
    /// Folder to watch
    dir: PathBuf,
    /// Folder for the PDFs. Defaults to `pdf` inside the watched folder
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Print every generated PDF
    #[arg(long)]
    print: bool,
    /// Printer name. Defaults to the system default printer
    #[arg(long, requires = "print")]
    printer: Option<String>,
    /// Number of printed copies
    #[arg(long, requires = "print", default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..=20))]
    copies: u32,
    #[command(flatten)]
    style: StyleArgs,
}

#[derive(Args)]
struct ConvertArgs {
    /// XML or ZIP files, or directories containing them
    #[arg(required = true)]
    inputs: Vec<PathBuf>,
    /// Output PDF file (single document) or output directory.
    /// Defaults to a PDF next to each input
    #[arg(short, long)]
    output: Option<PathBuf>,
    #[command(flatten)]
    style: StyleArgs,
}

#[derive(Args)]
struct StyleArgs {
    /// JSON configuration file; the options below override it
    #[arg(short, long)]
    config: Option<PathBuf>,
    /// Paper format: a4, a5, ticket80 or ticket58
    #[arg(short, long)]
    format: Option<PaperFormat>,
    /// Logo image (PNG, JPEG, GIF, WebP or SVG)
    #[arg(long)]
    logo: Option<PathBuf>,
    /// Custom Typst template replacing the built-in one
    #[arg(long)]
    template: Option<PathBuf>,
    /// Extra text printed at the end of the document
    #[arg(long)]
    footer: Option<String>,
    /// Do not print the QR code
    #[arg(long)]
    no_qr: bool,
}

impl StyleArgs {
    fn options(&self) -> Result<RenderOptions, String> {
        let mut options = match &self.config {
            Some(path) => config::load(path).map_err(|err| err.to_string())?,
            None => RenderOptions::default(),
        };
        if let Some(format) = self.format {
            options.settings.format = format;
        }
        if let Some(footer) = &self.footer {
            options.settings.footer = Some(footer.clone());
        }
        if self.no_qr {
            options.settings.show_qr = false;
        }
        if let Some(path) = &self.logo {
            options.logo = Some(config::load_logo(path).map_err(|err| err.to_string())?);
        }
        if let Some(path) = &self.template {
            options.template = Some(config::load_template(path).map_err(|err| err.to_string())?);
        }
        Ok(options)
    }
}

fn main() -> ExitCode {
    let result = match Cli::parse().command {
        Command::Convert(args) => convert(&args),
        Command::Template { format } => {
            print!("{}", xml2pdf_render::builtin_template(format));
            Ok(())
        }
        Command::Data { input, style } => data(&input, &style),
        Command::Watch(args) => watch_folder(&args),
        Command::Printers => printers(),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn convert(args: &ConvertArgs) -> Result<(), String> {
    let options = args.style.options()?;
    let inputs = expand_inputs(&args.inputs)?;
    if inputs.is_empty() {
        return Err("no XML or ZIP files found".into());
    }

    let output_is_file = args.output.as_deref().is_some_and(|path| {
        path.extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"))
    });
    if output_is_file && inputs.len() > 1 {
        return Err("`--output` must be a directory when converting several files".into());
    }
    if let Some(directory) = args.output.as_deref().filter(|_| !output_is_file) {
        std::fs::create_dir_all(directory)
            .map_err(|err| format!("cannot create `{}`: {err}", directory.display()))?;
    }

    let mut failures = 0;
    for input in &inputs {
        match convert_file(input, args.output.as_deref(), output_is_file, &options) {
            Ok(written) => written
                .iter()
                .for_each(|path| println!("{}", path.display())),
            Err(message) => {
                eprintln!("error: {message}");
                failures += 1;
            }
        }
    }

    match failures {
        0 => Ok(()),
        count => Err(format!("{count} of {} files failed", inputs.len())),
    }
}

/// Converts one input file and returns the PDFs written.
fn convert_file(
    input: &Path,
    output: Option<&Path>,
    output_is_file: bool,
    options: &RenderOptions,
) -> Result<Vec<PathBuf>, String> {
    let name = input.file_name().unwrap_or_default().to_string_lossy();
    let bytes = read(input)?;
    let rendered = xml2pdf_render::convert(&name, &bytes, options)
        .map_err(|err| format!("{}: {err}", input.display()))?;
    if output_is_file && rendered.len() > 1 {
        return Err(format!(
            "{}: contains {} documents, `--output` must be a directory",
            input.display(),
            rendered.len()
        ));
    }

    let single = rendered.len() == 1;
    let mut written = Vec::new();
    for document in rendered {
        // A lone document keeps the name of its input; documents inside an
        // archive are named after their identifier.
        let stem = if single {
            input
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        } else {
            document.file_stem
        };
        let path = match output {
            Some(file) if output_is_file => file.to_owned(),
            Some(directory) => directory.join(format!("{stem}.pdf")),
            None => input.with_file_name(format!("{stem}.pdf")),
        };
        std::fs::write(&path, document.pdf)
            .map_err(|err| format!("cannot write `{}`: {err}", path.display()))?;
        written.push(path);
    }
    Ok(written)
}

fn data(input: &Path, style: &StyleArgs) -> Result<(), String> {
    let options = style.options()?;
    let name = input.file_name().unwrap_or_default().to_string_lossy();
    let sources = xml2pdf_core::xml_sources(&name, &read(input)?).map_err(|err| err.to_string())?;
    for source in sources {
        let document = xml2pdf_core::parse_bytes(&source.bytes)
            .map_err(|err| format!("{}: {err}", source.name))?;
        let data = xml2pdf_render::template_data(&document, &options);
        println!(
            "{}",
            serde_json::to_string_pretty(&data).map_err(|err| err.to_string())?
        );
    }
    Ok(())
}

/// Watches a folder until the process is interrupted.
fn watch_folder(args: &WatchArgs) -> Result<(), String> {
    let options = args.style.options()?;
    let config = AutoConfig {
        watch_dir: Some(args.dir.clone()),
        output_dir: args.output.clone(),
        print: args.print,
        printer: args.printer.clone(),
        copies: args.copies,
        settings: options.settings.clone(),
        ..AutoConfig::default()
    };

    let _watcher = xml2pdf_auto::watch(config, options, Arc::new(SystemPrinter), report)
        .map_err(|err| err.to_string())?;
    eprintln!("Watching {} (Ctrl+C to stop)", args.dir.display());
    loop {
        std::thread::park();
    }
}

fn report(event: WatchEvent) {
    match event {
        WatchEvent::Processed(item) => match (item.printed, item.print_error) {
            (_, Some(error)) => eprintln!("error: {}: not printed: {error}", item.pdf.display()),
            (true, None) => println!("{} (printed)", item.pdf.display()),
            (false, None) => println!("{}", item.pdf.display()),
        },
        WatchEvent::Failed { source, message } => {
            eprintln!("error: {}: {message}", source.display())
        }
    }
}

fn printers() -> Result<(), String> {
    let names = SystemPrinter.list().map_err(|err| err.to_string())?;
    names.iter().for_each(|name| println!("{name}"));
    Ok(())
}

/// Replaces directories by the XML and ZIP files they contain, sorted by name.
fn expand_inputs(inputs: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for input in inputs {
        if !input.is_dir() {
            files.push(input.clone());
            continue;
        }
        let entries = std::fs::read_dir(input)
            .map_err(|err| format!("cannot read `{}`: {err}", input.display()))?;
        let mut found: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.is_file() && is_document(path))
            .collect();
        found.sort();
        files.append(&mut found);
    }
    Ok(files)
}

fn is_document(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("xml") || ext.eq_ignore_ascii_case("zip"))
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|err| format!("cannot read `{}`: {err}", path.display()))
}
