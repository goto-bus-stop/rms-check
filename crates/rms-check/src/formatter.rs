use crate::{
    parser::{Atom, Parser},
    wordize::Word,
};
use codespan::Files;
use std::iter::Peekable;

#[derive(Default)]
pub struct Formatter<'atom> {
    /// The current indentation level.
    indent: u32,
    /// Whether this line still needs indentation. A line needs indentation if no text has been
    /// written to it yet.
    needs_indent: bool,
    /// Width of commands.
    command_width: usize,
    /// Whether we are inside a command block.
    inside_block: usize,
    /// The formatted text.
    result: String,
    /// The last-written atom.
    prev: Option<Atom<'atom>>,
}

impl<'atom> Formatter<'atom> {
    /// Write a newline (Windows-style).
    fn newline(&mut self) {
        self.result.push_str("\r\n");
        self.needs_indent = true;
    }

    /// Indent the current line if it still needs it.
    fn maybe_indent(&mut self) {
        if self.needs_indent {
            for _ in 0..self.indent {
                self.result.push(' ');
            }
            self.needs_indent = false;
        }
    }

    /// Write some text to the current line.
    fn text(&mut self, text: &str) {
        self.maybe_indent();
        self.result.push_str(text);
    }

    /// Write a command.
    fn command<'w>(&mut self, name: &Word<'w>, args: &[Word<'w>], is_block: bool) {
        self.text(name.value);
        for _ in 0..self.command_width.saturating_sub(name.value.len()) {
            self.result.push(' ');
        }
        for arg in args {
            self.result.push(' ');
            self.text(arg.value);
        }
        if is_block {
            self.result.push(' ');
        } else {
            self.newline();
        }
    }

    /// Write a section header.
    fn section<'w>(&mut self, name: &Word<'w>) {
        if let Some(_) = self.prev {
            self.newline();
        }
        self.text(name.value);
        self.newline();
    }

    /// Write a command block. This reads atoms from the iterator until the end of the block, and
    /// writes both the command and any attributes it may contain.
    fn block<I>(&mut self, mut input: Peekable<I>) -> Peekable<I>
    where
        I: Iterator<Item = Atom<'atom>>,
    {
        use Atom::*;
        let is_end = |atom: &Atom<'_>| match atom {
            CloseBlock(_) => true,
            _ => false,
        };

        self.inside_block += 1;

        let mut commands = vec![];
        let mut longest = 0;
        let mut indent = 0;
        for atom in input.by_ref().take_while(|atom| !is_end(atom)) {
            longest = match &atom {
                Command(cmd, _) => longest.max(cmd.value.len() + indent),
                If(_, _) => {
                    indent += 2;
                    longest
                }
                EndIf(_) => {
                    indent -= 2;
                    longest
                }
                _ => longest,
            };
            commands.push(atom);
        }
        self.text("{");
        self.newline();
        self.indent += 2;

        let old = self.command_width;
        self.command_width = longest;
        let mut sub_input = commands.into_iter().peekable();
        while let Some(atom) = sub_input.next() {
            sub_input = self.write_atom(atom, sub_input);
        }
        self.command_width = old;

        self.inside_block -= 1;

        self.indent -= 2;
        self.text("}");
        self.newline();

        input
    }

    fn condition<I>(&mut self, cond: &Word<'_>, mut input: Peekable<I>) -> Peekable<I>
    where
        I: Iterator<Item = Atom<'atom>>,
    {
        use Atom::*;

        self.text("if ");
        self.text(cond.value);
        self.newline();
        self.indent += 2;

        // reset command width so an if block within a command block
        // does not over-indent.
        let old_command_width = self.command_width;
        self.command_width = 0;

        let mut depth = 1;
        let content: Vec<Atom<'atom>> = input
            .by_ref()
            .take_while(|atom| {
                match atom {
                    If(_, _) => depth += 1,
                    EndIf(_) => depth -= 1,
                    _ => (),
                }

                match atom {
                    EndIf(_) if depth == 0 => false,
                    _ => true,
                }
            })
            .collect();

        let mut sub_input = content.into_iter().peekable();
        while let Some(atom) = sub_input.next() {
            match atom {
                Atom::ElseIf(_, cond) => {
                    self.indent -= 2;
                    self.text("elseif ");
                    self.text(cond.value);
                    self.newline();
                    self.indent += 2;
                }
                Atom::Else(_) => {
                    self.indent -= 2;
                    self.text("else");
                    self.newline();
                    self.indent += 2;
                }
                _ => {
                    sub_input = self.write_atom(atom, sub_input);
                }
            }
        }

        self.command_width = old_command_width;

        self.indent -= 2;
        self.text("endif");
        self.newline();

        if self.inside_block == 0 {
            self.newline();
        }

        input
    }

    fn random<I>(&mut self, mut input: Peekable<I>) -> Peekable<I>
    where
        I: Iterator<Item = Atom<'atom>>,
    {
        use Atom::*;

        self.text("start_random");
        self.newline();
        self.indent += 2;

        // reset command width so a start_random within a command block
        // does not over-indent.
        let old_command_width = self.command_width;
        self.command_width = 0;

        let mut null_branch = vec![];
        let mut branches = vec![];
        let mut depth = 1;
        for atom in input.by_ref() {
            match &atom {
                PercentChance(_, arg) if depth == 1 => {
                    branches.push((arg.clone(), vec![]));
                    continue;
                }
                StartRandom(_) => {
                    depth += 1;
                }
                EndRandom(_) => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => (),
            };

            if branches.len() > 0 {
                branches.last_mut().unwrap().1.push(atom);
            } else {
                null_branch.push(atom);
            }
        }

        let has_simple_branches = branches.iter().all(|(_, stmts)| {
            if stmts.len() > 1 {
                return false;
            }
            if stmts.len() == 0 {
                return true;
            }
            match stmts[0] {
                Define(_, _) => true,
                Const(_, _, _) => true,
                // Include(_, _) => true,
                // IncludeDrs(_, _) => true,
                Command(_, _) => true,
                _ => false,
            }
        });

        if has_simple_branches {
            let longest = branches.iter().fold(0, |acc, (chance, _)| {
                acc.max(format!("percent_chance {}", chance.value).len())
            });
            for (chance, mut branch) in branches {
                let mut chance = format!("percent_chance {}", chance.value);
                while chance.len() < longest {
                    chance.push(' ');
                }
                self.text(&chance);
                if branch.len() > 0 {
                    self.text(" ");
                    input = self.write_atom(branch.remove(0), input);
                }
            }
        }

        self.command_width = old_command_width;

        self.indent -= 2;
        self.text("end_random");
        self.newline();

        input
    }

    /// Write a comment. Multiline comments are formatted Java-style, with a * at the start of each
    /// line.
    fn comment(&mut self, content: &str) {
        self.text("/* ");
        let mut lines = content.lines();
        if let Some(first_line) = lines.next() {
            self.text(first_line.trim());
        }
        let mut is_multiline = false;
        for line in lines {
            is_multiline = true;
            self.newline();
            self.text(" * ");
            if line.trim().starts_with("* ") {
                self.text(
                    &line
                        .chars()
                        .skip_while(|&c| char::is_whitespace(c))
                        .collect::<String>(),
                );
            } else {
                self.text(line);
            }
        }
        if is_multiline {
            self.newline();
        }
        self.text(" */");
        self.newline();
    }

    /// Write a #define statement.
    fn define(&mut self, name: &Word<'_>) {
        self.text("#define ");
        self.text(name.value);
        self.newline();
    }

    /// Write a #const statement.
    fn const_(&mut self, name: &Word<'_>, value: &Option<Word<'_>>) {
        self.text("#const ");
        self.text(name.value);
        self.text(" ");
        if let Some(value) = value {
            self.text(value.value);
        }
        self.newline();
    }

    fn write_atom<I>(&mut self, atom: Atom<'atom>, mut input: Peekable<I>) -> Peekable<I>
    where
        I: Iterator<Item = Atom<'atom>>,
    {
        use Atom::*;

        if let Some(CloseBlock(_)) = self.prev {
            self.newline();
        }

        match &atom {
            Section(name) => self.section(name),
            Define(_, name) => self.define(name),
            Const(_, name, value) => self.const_(name, value),
            Command(name, args) => {
                let is_block = if let Some(OpenBlock(_)) = input.peek() {
                    true
                } else {
                    false
                };
                self.command(name, args, is_block);
            }
            OpenBlock(_) => {
                input = self.block(input);
            }
            If(_, cond) => {
                input = self.condition(cond, input);
            }
            StartRandom(_) => {
                input = self.random(input);
            }
            Comment(_, content, _) => self.comment(content),
            _ => (),
        }
        self.prev = Some(atom);
        input
    }

    /// Format a script. Takes an iterator over atoms.
    pub fn format(mut self, input: impl Iterator<Item = Atom<'atom>>) -> String {
        let mut input = input.peekable();
        while let Some(atom) = input.next() {
            input = self.write_atom(atom, input);
        }
        self.result
    }
}

/// Format an rms source string.
pub fn format(source: &str) -> String {
    let mut files = Files::new();
    let f = files.add("format.rms", source);
    let parser = Parser::new(f, files.source(f));
    Formatter::default().format(parser.map(|(atom, _)| atom))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_section() {
        assert_eq!(
            format("<PLAYER_SETUP> <OBJECTS_GENERATION>"),
            "<PLAYER_SETUP>\r\n\r\n<OBJECTS_GENERATION>\r\n"
        );
    }

    #[test]
    fn command_group() {
        assert_eq!(
            format("create_terrain GRASS3 { base_terrain DESERT border_fuzziness 5 }"),
            "create_terrain GRASS3 {\r\n  base_terrain     DESERT\r\n  border_fuzziness 5\r\n}\r\n"
        );
    }
}
