//! In-memory Typst world: one template, its data files and the bundled fonts.

use std::collections::HashMap;
use std::sync::OnceLock;

use typst::diag::{FileError, FileResult, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World, WorldExt};

const MAIN: &str = "main.typ";

static FONT_FILES: [&[u8]; 2] = [
    include_bytes!("../fonts/NotoSans-Regular.ttf"),
    include_bytes!("../fonts/NotoSans-Bold.ttf"),
];

struct Fonts {
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
}

fn fonts() -> &'static Fonts {
    static FONTS: OnceLock<Fonts> = OnceLock::new();
    FONTS.get_or_init(|| {
        let fonts: Vec<Font> = FONT_FILES
            .iter()
            .flat_map(|data| Font::iter(Bytes::new(*data)))
            .collect();
        Fonts {
            book: LazyHash::new(FontBook::from_fonts(&fonts)),
            fonts,
        }
    })
}

fn library() -> &'static LazyHash<Library> {
    static LIBRARY: OnceLock<LazyHash<Library>> = OnceLock::new();
    LIBRARY.get_or_init(|| LazyHash::new(<Library as LibraryExt>::default()))
}

pub struct TemplateWorld {
    main: Source,
    files: HashMap<String, Bytes>,
}

impl TemplateWorld {
    /// Creates a world whose entry point is `template`. `files` are addressed
    /// from the template by their relative path, for example `data.json`.
    pub fn new(template: String, files: impl IntoIterator<Item = (String, Vec<u8>)>) -> Self {
        Self {
            main: Source::new(file_id(MAIN), template),
            files: files
                .into_iter()
                .map(|(path, data)| (path, Bytes::new(data)))
                .collect(),
        }
    }

    /// Describes a diagnostic as `line N: message (hint)`.
    pub fn describe(&self, diagnostic: &SourceDiagnostic) -> String {
        let mut message = diagnostic.message.to_string();
        if let Some(range) = self.range(diagnostic.span) {
            let line = self.main.text()[..range.start.min(self.main.text().len())]
                .bytes()
                .filter(|&byte| byte == b'\n')
                .count()
                + 1;
            message = format!("line {line}: {message}");
        }
        for hint in &diagnostic.hints {
            message.push_str(&format!(" ({})", hint.v));
        }
        message
    }
}

fn file_id(path: &str) -> FileId {
    let path = VirtualPath::new(path).expect("built-in virtual paths are valid");
    FileId::new(RootedPath::new(VirtualRoot::Project, path))
}

impl World for TemplateWorld {
    fn library(&self) -> &LazyHash<Library> {
        library()
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &fonts().book
    }

    fn main(&self) -> FileId {
        self.main.id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main.id() {
            Ok(self.main.clone())
        } else {
            Err(FileError::NotFound(
                id.get().vpath().get_without_slash().into(),
            ))
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        let path = id.get().vpath().get_without_slash();
        match (id.get().root(), self.files.get(path)) {
            (VirtualRoot::Project, Some(data)) => Ok(data.clone()),
            _ => Err(FileError::NotFound(path.into())),
        }
    }

    fn font(&self, index: usize) -> Option<Font> {
        fonts().fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}
