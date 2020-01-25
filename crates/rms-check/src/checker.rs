use crate::parser::{Atom, AtomKind, Parser};
use crate::tokens::TokenType;
use crate::wordize::Word;
use crate::RMSFile;
use codespan::{ByteIndex, FileId, Span};
pub use codespan_reporting::diagnostic::{Diagnostic, Label, Severity};
use lazy_static::lazy_static;
use std::collections::HashSet;

/// The target compatibility for a map script.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub enum Compatibility {
    /// The Conquerors.
    Conquerors = 1,
    /// Target UserPatch 1.4, accept the features it added.
    UserPatch14 = 3,
    /// Target UserPatch 1.5, accept the features it added.
    UserPatch15 = 4,
    /// Target WololoKingdoms: use UserPatch 1.5, constants for HD Edition DLC units and terrains,
    /// and auto-use UserPatch-specific constants.
    WololoKingdoms = 5,
    /// Target HD Edition (assumes all DLCs).
    HDEdition = 2,
    /// Target Definitive Edition.
    DefinitiveEdition = 6,
    /// Try to be maximally compatible. This is basically the same as targeting Conquerors.
    All = 0,
}

impl Default for Compatibility {
    #[inline]
    fn default() -> Compatibility {
        Compatibility::Conquerors
    }
}

#[derive(Debug, Clone)]
pub enum AutoFixReplacement {
    None,
    Safe(String),
    Unsafe(String),
}

impl AutoFixReplacement {
    #[inline]
    pub fn is_fixable(&self) -> bool {
        match self {
            AutoFixReplacement::Safe(_) => true,
            _ => false,
        }
    }
    #[inline]
    pub fn is_fixable_unsafe(&self) -> bool {
        match self {
            AutoFixReplacement::None => false,
            _ => true,
        }
    }
}

/// A suggestion that may fix a warning.
#[derive(Debug, Clone)]
pub struct Suggestion {
    /// The file this suggestion applies to.
    file_id: FileId,
    /// The piece of source code that this suggestion would replace.
    span: Span,
    /// Human-readable suggestion message.
    message: String,
    /// A replacement string that could fix the problem.
    replacement: AutoFixReplacement,
}

impl Suggestion {
    /// Get the codespan file ID that would be updated by this suggestion.
    #[inline]
    pub const fn file_id(&self) -> FileId {
        self.file_id
    }
    /// Get the span this suggestion applies to.
    #[inline]
    pub const fn span(&self) -> Span {
        self.span
    }
    /// Get the starting position this suggestion applies to.
    #[inline]
    pub fn start(&self) -> ByteIndex {
        self.span.start()
    }
    /// Get the end position this suggestion applies to.
    #[inline]
    pub fn end(&self) -> ByteIndex {
        self.span.end()
    }
    /// Get the suggestion message.
    #[inline]
    pub fn message(&self) -> &str {
        &self.message
    }
    /// Get the replacement string that could fix the problem.
    #[inline]
    pub const fn replacement(&self) -> &AutoFixReplacement {
        &self.replacement
    }

    /// Create a suggestion.
    #[inline]
    pub fn new(file_id: FileId, span: Span, message: impl ToString) -> Self {
        let message = message.to_string();
        Suggestion {
            file_id,
            span,
            message,
            replacement: AutoFixReplacement::None,
        }
    }
    /// Create a suggestion applying to a specific token.
    #[inline]
    pub fn from(token: &Word<'_>, message: impl ToString) -> Self {
        let message = message.to_string();
        Suggestion {
            file_id: token.file,
            span: token.span,
            message,
            replacement: AutoFixReplacement::None,
        }
    }
    /// Specify a possible fix for the problem.
    #[inline]
    pub fn replace(mut self, replacement: impl ToString) -> Self {
        self.replacement = AutoFixReplacement::Safe(replacement.to_string());
        self
    }
    /// Specify a possible fix for the problem, but one that may not be correct and requires some
    /// manual intervention.
    #[inline]
    pub fn replace_unsafe(mut self, replacement: impl ToString) -> Self {
        self.replacement = AutoFixReplacement::Unsafe(replacement.to_string());
        self
    }
}

/// A warning.
#[derive(Debug, Clone)]
pub struct Warning {
    diagnostic: Diagnostic,
    /// A change suggestion: when present, the problem can be fixed by replacing the
    /// range of text this warning applies to by the string in this suggestion.
    suggestions: Vec<Suggestion>,
}

impl Warning {
    /// Get the diagnostic for this warning.
    #[inline]
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }
    /// Get the severity of this warning.
    #[inline]
    pub const fn severity(&self) -> Severity {
        self.diagnostic.severity
    }
    /// Get additional labels for this warning.
    #[inline]
    pub const fn labels(&self) -> &Vec<Label> {
        &self.diagnostic.secondary_labels
    }
    /// Get the human-readable error message.
    #[inline]
    pub fn message(&self) -> &str {
        &self.diagnostic.message
    }
    /// Check whether any suggestions could be made.
    #[inline]
    pub fn has_suggestions(&self) -> bool {
        !self.suggestions.is_empty()
    }
    /// Get any suggestions that may help to fix the problem.
    #[inline]
    pub const fn suggestions(&self) -> &Vec<Suggestion> {
        &self.suggestions
    }

    /// Create a new warning with severity "Warning".
    #[allow(unused)]
    #[must_use]
    pub(crate) fn warning(file_id: FileId, span: Span, message: impl Into<String>) -> Self {
        let message: String = message.into();
        Warning {
            diagnostic: Diagnostic::new_warning(
                message.clone(),
                Label::new(file_id, span, message),
            ),
            suggestions: vec![],
        }
    }

    /// Create a new warning with severity "Error".
    #[must_use]
    pub(crate) fn error(file_id: FileId, span: Span, message: impl Into<String>) -> Self {
        let message: String = message.into();
        Warning {
            diagnostic: Diagnostic::new_error(message.clone(), Label::new(file_id, span, message)),
            suggestions: vec![],
        }
    }

    /// Define a replacement suggestion for this warning.
    pub(crate) fn suggest(mut self, suggestion: Suggestion) -> Self {
        self.suggestions.push(suggestion);
        self
    }

    /// Add a note referencing a snippet of code.
    pub(crate) fn note_at(mut self, file_id: FileId, span: Span, message: &str) -> Self {
        self.diagnostic = self
            .diagnostic
            .with_secondary_labels(vec![Label::new(file_id, span, message)]);
        self
    }

    /// Set the lint that emitted this warning.
    pub(crate) fn lint(mut self, lint: &str) -> Self {
        self.diagnostic = self.diagnostic.with_code(lint);
        self
    }
}

impl Word<'_> {
    /// Create a warning applying to this token.
    #[must_use]
    pub(crate) fn warning(&self, message: impl Into<String>) -> Warning {
        let message: String = message.into();
        Warning {
            diagnostic: Diagnostic::new_warning(
                message.clone(),
                Label::new(self.file, self.span, message),
            ),
            suggestions: vec![],
        }
    }
    /// Create an error applying to this token.
    #[must_use]
    pub(crate) fn error(&self, message: impl Into<String>) -> Warning {
        let message: String = message.into();
        Warning {
            diagnostic: Diagnostic::new_error(
                message.clone(),
                Label::new(self.file, self.span, message),
            ),
            suggestions: vec![],
        }
    }
}

impl Atom<'_> {
    /// Create a warning applying to this token.
    #[must_use]
    pub(crate) fn warning(&self, message: impl Into<String>) -> Warning {
        let message: String = message.into();
        Warning {
            diagnostic: Diagnostic::new_warning(
                message.clone(),
                Label::new(self.file, self.span, message),
            ),
            suggestions: vec![],
        }
    }
    /// Create an error applying to this token.
    #[must_use]
    pub(crate) fn error(&self, message: impl Into<String>) -> Warning {
        let message: String = message.into();
        Warning {
            diagnostic: Diagnostic::new_error(
                message.clone(),
                Label::new(self.file, self.span, message),
            ),
            suggestions: vec![],
        }
    }
}

#[derive(Debug, Clone)]
pub enum Nesting<'a> {
    If(Atom<'a>),
    ElseIf(Atom<'a>),
    Else(Atom<'a>),
    StartRandom(Atom<'a>),
    PercentChance(Atom<'a>),
    Brace(Atom<'a>),
}

pub trait Lint {
    fn name(&self) -> &'static str;
    fn run_inside_comments(&self) -> bool {
        false
    }
    fn lint_atom(&mut self, _state: &mut ParseState<'_>, _atom: &Atom<'_>) -> Vec<Warning> {
        Default::default()
    }
}

#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq)]
enum HeaderName {
    Compatibility,
}

fn parse_header_name(name: &str) -> Option<HeaderName> {
    match name.to_ascii_lowercase().trim() {
        "compatibility" => Some(HeaderName::Compatibility),
        _ => None,
    }
}

#[derive(Debug)]
pub struct ParseState<'a> {
    /// The files.
    pub rms: &'a RMSFile,
    /// The target compatibility for this map script.
    pub compatibility: Compatibility,
    /// Whether this map should be treated as a builtin map. If true, #include and #include_drs should be made available.
    pub is_builtin_map: bool,
    /// The amount of nested statements we entered, like `if`, `start_random`.
    pub nesting: Vec<Nesting<'a>>,
    /// The token type that we are currently reading arguments for.
    pub current_token: Option<&'static TokenType>,
    /// The current <SECTION>, as well as its opening token.
    pub current_section: Option<Atom<'a>>,
    /// List of builtin #const definitions.
    builtin_consts: HashSet<String>,
    /// List of builtin #define definitions.
    builtin_defines: HashSet<String>,
    /// List of user-mode #const definitions we've seen so far.
    pub seen_consts: HashSet<String>,
    /// List of user-mode #define definitions we've seen so far.
    pub seen_defines: HashSet<String>,
    /// List of builtin optional definitions.
    pub option_defines: HashSet<String>,
    /// Are we still parsing header comments?
    end_of_headers: bool,
}

impl<'a> ParseState<'a> {
    fn new(rms: &'a RMSFile, compatibility: Compatibility) -> Self {
        let mut state = Self {
            rms,
            compatibility,
            is_builtin_map: false,
            nesting: vec![],
            current_token: None,
            current_section: None,
            builtin_consts: HashSet::new(),
            builtin_defines: HashSet::new(),
            seen_consts: HashSet::new(),
            seen_defines: HashSet::new(),
            option_defines: HashSet::new(),
            end_of_headers: false,
        };
        state.set_compatibility(compatibility);
        state
    }

    /// Track that a `#define` name may or may not exist from this point.
    ///
    /// These defines are valid in `if` statements, but not in commands, for example.
    pub fn optional_define(&mut self, name: impl ToString) {
        self.option_defines.insert(name.to_string());
    }
    /// Track that a `#define` name exists.
    pub fn define(&mut self, name: impl ToString) {
        self.seen_defines.insert(name.to_string());
    }
    /// Track that a `#const` name exists.
    pub fn define_const(&mut self, name: impl ToString) {
        self.seen_consts.insert(name.to_string());
    }
    /// Does a given `#define` name exist?
    pub fn has_define(&self, name: &str) -> bool {
        self.seen_defines.contains(name) || self.builtin_defines.contains(name)
    }
    /// May a given `#define` name exist at this point?
    pub fn may_have_define(&self, name: &str) -> bool {
        self.has_define(name) || self.option_defines.contains(name)
    }
    /// Does a given `#const` name exist?
    pub fn has_const(&self, name: &str) -> bool {
        self.seen_consts.contains(name) || self.builtin_consts.contains(name)
    }
    /// List all the `#const` names that are currently available.
    pub fn consts(&self) -> impl Iterator<Item = &str> {
        self.seen_consts
            .iter()
            .map(|string| string.as_ref())
            .chain(self.builtin_consts.iter().map(|string| string.as_ref()))
    }
    /// List all the `#define` names that are currently available.
    pub fn defines(&self) -> impl Iterator<Item = &str> {
        self.seen_defines
            .iter()
            .map(|string| string.as_ref())
            .chain(self.builtin_defines.iter().map(|string| string.as_ref()))
    }

    /// Get the compatibility mode the parser runs in.
    pub const fn compatibility(&self) -> Compatibility {
        self.compatibility
    }

    /// Set the compatibility mode the parser should run in.
    ///
    /// This affects the available builtin `#define` and `#const` names.
    pub fn set_compatibility(&mut self, compatibility: Compatibility) {
        self.compatibility = compatibility;

        self.builtin_consts.clear();
        self.builtin_defines.clear();

        let (file_id, content) = self.rms.definitions(compatibility);

        for (atom, _) in Parser::new(file_id, content) {
            match atom.kind {
                AtomKind::Const { name, .. } => {
                    self.builtin_consts.insert(name.value.to_string());
                }
                AtomKind::Define { name, .. } => {
                    self.builtin_defines.insert(name.value.to_string());
                }
                _ => (),
            }
        }
    }

    /// Update the parse state upon reading a new Atom.
    fn update(&mut self, atom: &Atom<'a>) {
        self.update_headers(atom);

        match atom.kind {
            AtomKind::Section { .. } => {
                self.current_section = Some(atom.clone());
            }
            AtomKind::Define { name, .. } => {
                self.define(name.value);
            }
            AtomKind::Const { name, .. } => {
                self.define_const(name.value);
            }
            _ => (),
        }
    }

    fn set_header(&mut self, name: HeaderName, value: &str) {
        match name {
            HeaderName::Compatibility => {
                let compat = match value.to_ascii_lowercase().trim() {
                    "hd edition" | "hd" => Compatibility::HDEdition,
                    "conquerors" | "aoc" => Compatibility::Conquerors,
                    "userpatch 1.5" | "up 1.5" => Compatibility::UserPatch15,
                    "userpatch 1.4" | "up 1.4" | "userpatch" | "up" => Compatibility::UserPatch14,
                    "wololokingdoms" | "wk" => Compatibility::WololoKingdoms,
                    "definitive edition" | "de" => Compatibility::DefinitiveEdition,
                    _ => return,
                };
                self.set_compatibility(compat);
            }
        }
    }

    fn parse_header_comment(&mut self, content: &str) {
        for mut line in content.lines() {
            line = line.trim();
            if line.starts_with("* ") {
                line = &line[2..];
            }

            let mut parts = line.splitn(2, ": ");
            if let (Some(name), Some(val)) = (parts.next(), parts.next()) {
                if let Some(header) = parse_header_name(name) {
                    self.set_header(header, val);
                }
            }
        }
    }

    fn update_headers(&mut self, atom: &Atom<'a>) {
        if self.end_of_headers {
            return;
        }
        if let AtomKind::Comment { content, .. } = &atom.kind {
            self.parse_header_comment(content);
        } else {
            self.end_of_headers = true;
        }
    }

    /// Update the nesting state upon reading a new Atom.
    fn update_nesting(&mut self, atom: &Atom<'a>) -> Option<Warning> {
        fn unbalanced_error(name: &str, end: &Atom<'_>, nest: Option<&Nesting<'_>>) -> Warning {
            let msg = format!("Unbalanced `{}`", name);
            match nest {
                Some(Nesting::Brace(start)) => {
                    end.error(msg)
                        .note_at(start.file, start.span, "Matches this open brace `{`")
                }
                Some(Nesting::If(start)) => {
                    end.error(msg)
                        .note_at(start.file, start.span, "Matches this `if`")
                }
                Some(Nesting::ElseIf(start)) => {
                    end.error(msg)
                        .note_at(start.file, start.span, "Matches this `elseif`")
                }
                Some(Nesting::Else(start)) => {
                    end.error(msg)
                        .note_at(start.file, start.span, "Matches this `else`")
                }
                Some(Nesting::StartRandom(start)) => {
                    end.error(msg)
                        .note_at(start.file, start.span, "Matches this `start_random`")
                }
                Some(Nesting::PercentChance(start)) => {
                    end.error(msg)
                        .note_at(start.file, start.span, "Matches this `percent_chance`")
                }
                None => end.error(format!("{}–nothing is open", msg)),
            }
        }

        match atom.kind {
            AtomKind::OpenBlock { .. } => {
                self.nesting.push(Nesting::Brace(atom.clone()));
            }
            AtomKind::CloseBlock { .. } => match self.nesting.last() {
                Some(Nesting::Brace(_)) => {
                    self.nesting.pop();
                }
                nest => {
                    return Some(unbalanced_error("}", atom, nest));
                }
            },
            AtomKind::If { .. } => self.nesting.push(Nesting::If(atom.clone())),
            AtomKind::ElseIf { .. } => {
                match self.nesting.last() {
                    Some(Nesting::If(_)) | Some(Nesting::ElseIf(_)) => {
                        self.nesting.pop();
                    }
                    nest => {
                        return Some(unbalanced_error("elseif", atom, nest));
                    }
                }
                self.nesting.push(Nesting::ElseIf(atom.clone()));
            }
            AtomKind::Else { .. } => {
                match self.nesting.last() {
                    Some(Nesting::If(_)) | Some(Nesting::ElseIf(_)) => {
                        self.nesting.pop();
                    }
                    nest => {
                        return Some(unbalanced_error("else", atom, nest));
                    }
                }
                self.nesting.push(Nesting::Else(atom.clone()));
            }
            AtomKind::EndIf { .. } => match self.nesting.last() {
                Some(Nesting::If(_)) | Some(Nesting::ElseIf(_)) | Some(Nesting::Else(_)) => {
                    self.nesting.pop();
                }
                nest => {
                    return Some(unbalanced_error("endif", atom, nest));
                }
            },
            AtomKind::StartRandom { .. } => self.nesting.push(Nesting::StartRandom(atom.clone())),
            AtomKind::PercentChance { .. } => {
                if let Some(Nesting::PercentChance(_)) = self.nesting.last() {
                    self.nesting.pop();
                }

                match self.nesting.last() {
                    Some(Nesting::StartRandom(_)) => {}
                    nest => {
                        return Some(unbalanced_error("percent_chance", atom, nest));
                    }
                }

                self.nesting.push(Nesting::PercentChance(atom.clone()));
            }
            AtomKind::EndRandom { .. } => {
                if let Some(Nesting::PercentChance(_)) = self.nesting.last() {
                    self.nesting.pop();
                };

                match self.nesting.last() {
                    Some(Nesting::StartRandom(_)) => {
                        self.nesting.pop();
                    }
                    nest => {
                        return Some(unbalanced_error("end_random", atom, nest));
                    }
                }
            }
            _ => (),
        }

        None
    }
}

/// Builtin #define or #const names for AoE2: The Age of Conquerors.
const AOC_OPTION_DEFINES: [&str; 8] = [
    "TINY_MAP",
    "SMALL_MAP",
    "MEDIUM_MAP",
    "LARGE_MAP",
    "HUGE_MAP",
    "GIGANTIC_MAP",
    "UP_AVAILABLE",
    "UP_EXTENSION",
];

lazy_static! {
    /// Builtin #define or #const names for UserPatch.
    static ref UP_OPTION_DEFINES: Vec<String> = {
        let mut list = vec![
            "FIXED_POSITIONS".to_string(),
            "AI_PLAYERS".to_string(),
            "CAPTURE_RELIC".to_string(),
            "DEATH_MATCH".to_string(),
            "DEFEND_WONDER".to_string(),
            "KING_OT_HILL".to_string(),
            "RANDOM_MAP".to_string(),
            "REGICIDE".to_string(),
            "TURBO_RANDOM_MAP".to_string(),
            "WONDER_RACE".to_string(),
        ];

        for i in 1..=8 {
            list.push(format!("{}_PLAYER_GAME", i));
        }
        for i in 0..=4 {
            list.push(format!("{}_TEAM_GAME", i));
        }
        for team in 0..=4 {
            for player in 1..=8 {
                list.push(format!("PLAYER{}_TEAM{}", player, team));
            }
        }
        for team in 0..=4 {
            for size in 0..=8 {
                list.push(format!("TEAM{}_SIZE{}", team, size));
            }
        }

        list
    };
}

#[derive(Default)]
pub struct CheckerBuilder {
    lints: Vec<Box<dyn Lint>>,
    compatibility: Compatibility,
}

impl CheckerBuilder {
    pub fn build(self, rms: &RMSFile) -> Checker<'_> {
        // Default to UP 1.5 if it's a ZR@ map
        let compatibility = if rms.is_zip_rms() && self.compatibility < Compatibility::UserPatch15 {
            Compatibility::UserPatch15
        } else {
            self.compatibility
        };

        let state = ParseState::new(rms, compatibility);
        Checker {
            lints: self.lints,
            state,
        }
    }

    pub fn with_lint(mut self, lint: Box<dyn Lint>) -> Self {
        self.lints.push(lint);
        self
    }

    pub const fn compatibility(mut self, compatibility: Compatibility) -> Self {
        self.compatibility = compatibility;
        self
    }
}

pub struct Checker<'a> {
    lints: Vec<Box<dyn Lint>>,
    state: ParseState<'a>,
}

impl<'a> Checker<'a> {
    pub fn builder() -> CheckerBuilder {
        CheckerBuilder::default()
    }

    pub fn write_atom(&mut self, atom: &Atom<'a>) -> Vec<Warning> {
        let mut state = &mut self.state;
        let mut warnings = vec![];
        for lint in self.lints.iter_mut() {
            let new_warnings = lint
                .lint_atom(&mut state, atom)
                .into_iter()
                .map(move |warning| warning.lint(lint.name()));
            warnings.extend(new_warnings);
        }

        self.state.update(atom);
        if let Some(nest_warning) = self.state.update_nesting(atom) {
            warnings.push(nest_warning);
        }

        warnings
    }
}
