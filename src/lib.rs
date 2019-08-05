mod checker;
mod lints;
mod parser;
mod tokens;
mod wordize;

use crate::checker::Checker;
use crate::wordize::Wordize;
use codespan::{ByteIndex, ByteOffset, CodeMap, ColumnIndex, FileMap, FileName, LineIndex};
use std::io::Result;
use std::path::PathBuf;
use std::sync::Arc;

pub use crate::checker::{
    AutoFixReplacement, Compatibility, Expect, Lint, Nesting, ParseState, Severity, Suggestion,
    Warning,
};
pub use crate::tokens::{ArgType, TokenContext, TokenType, TOKENS};
pub use crate::wordize::Word;

pub struct RMSCheckResult {
    warnings: Vec<Warning>,
    codemap: CodeMap,
}

impl RMSCheckResult {
    pub fn codemap(&self) -> &CodeMap {
        &self.codemap
    }

    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    pub fn resolve_position(&self, index: ByteIndex) -> Option<(LineIndex, ColumnIndex)> {
        let file = self.codemap.find_file(index);
        file.and_then(|f| f.location(index).ok())
    }

    pub fn resolve_offset(&self, index: ByteIndex) -> Option<ByteOffset> {
        let file = self.codemap.find_file(index);
        file.and_then(|f| {
            f.location(index)
                .ok()
                .and_then(|(l, c)| f.offset(l, c).ok())
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = &Warning> {
        self.warnings.iter()
    }
}

pub struct RMSCheck<'a> {
    checker: Checker<'a>,
    codemap: CodeMap,
    filemaps: Vec<Arc<FileMap>>,
}

impl<'a> Default for RMSCheck<'a> {
    fn default() -> RMSCheck<'a> {
        RMSCheck::new()
            .with_lint(Box::new(lints::AttributeCaseLint {}))
            .with_lint(Box::new(lints::CommentContentsLint::new()))
            .with_lint(Box::new(lints::CompatibilityLint::new()))
            .with_lint(Box::new(lints::DeadBranchCommentLint {}))
            .with_lint(Box::new(lints::IncludeLint::new()))
            .with_lint(Box::new(lints::IncorrectSectionLint {}))
        // .with_lint(Box::new(lints::UnknownAttributeLint {}))
        // buggy
    }
}

impl<'a> RMSCheck<'a> {
    pub fn new() -> Self {
        let check = RMSCheck {
            checker: Checker::default(),
            codemap: CodeMap::new(),
            filemaps: vec![],
        };
        check.add_source("random_map.def", include_str!("random_map.def"))
    }

    pub fn compatibility(self, compatibility: Compatibility) -> Self {
        Self {
            checker: self.checker.compatibility(compatibility),
            ..self
        }
    }

    pub fn with_lint(self, lint: Box<Lint>) -> Self {
        Self {
            checker: self.checker.with_lint(lint),
            ..self
        }
    }

    pub fn add_source(mut self, name: &str, source: &str) -> Self {
        let map = self.codemap.add_filemap(
            FileName::Virtual(name.to_string().into()),
            source.to_string(),
        );
        self.filemaps.push(map);
        self
    }

    pub fn add_file(mut self, path: PathBuf) -> Result<Self> {
        let map = self.codemap.add_filemap_from_disk(path)?;
        self.filemaps.push(map);
        Ok(self)
    }

    pub fn codemap(&self) -> &CodeMap {
        &self.codemap
    }

    pub fn check(self) -> RMSCheckResult {
        let mut checker = self.checker.build();
        let words = self.filemaps.iter().map(|map| Wordize::new(&map)).flatten();

        let warnings = words.filter_map(|w| checker.write_token(&w)).collect();

        RMSCheckResult {
            codemap: self.codemap,
            warnings,
        }
    }
}

/// Check a random map script for errors or other issues.
pub fn check(source: &str, compatibility: Compatibility) -> RMSCheckResult {
    let checker = RMSCheck::default()
        .compatibility(compatibility)
        .add_source("source.rms", source);

    checker.check()
}
