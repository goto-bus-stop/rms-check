use ansi_term::{Colour, Style};
use rms_check::{AtomKind, ByteIndex, Parser, RMSFile, Word};
use std::io::{Cursor, Result, Write};

struct Printer<'a> {
    source: &'a str,
    last_index: Option<ByteIndex>,
}

impl<'a> Printer<'a> {
    fn print_whitespace(&mut self, index: ByteIndex, mut output: impl Write) -> Result<()> {
        let last_index = self.last_index.take().unwrap_or_else(|| ByteIndex::from(0));
        let slice = &self.source[usize::from(last_index)..usize::from(index)];
        output.write_all(slice.as_bytes())?;
        Ok(())
    }

    fn print_string(
        &mut self,
        string: &str,
        end: ByteIndex,
        style: Style,
        mut output: impl Write,
    ) -> Result<()> {
        write!(output, "{}", style.paint(string))?;
        self.last_index = Some(end);
        Ok(())
    }

    fn print_word(&mut self, word: Word<'a>, style: Style, mut output: impl Write) -> Result<()> {
        let start_index = word.location.start();
        self.print_whitespace(start_index, &mut output)?;

        write!(output, "{}", style.paint(word.value))?;
        self.last_index = Some(word.location.end());

        Ok(())
    }
}

fn highlight_atoms_to(source: &str, parser: Parser, mut output: impl Write) -> Result<()> {
    let mut printer = Printer {
        source,
        last_index: None,
    };

    let color_attribute: Style = Colour::White.normal();
    let color_comment: Style = Colour::Fixed(71).italic();
    let color_const: Style = Colour::Fixed(51).bold();
    let color_keyword: Style = Colour::Yellow.normal();
    let color_number: Style = Colour::Purple.normal();
    let color_section: Style = Colour::Fixed(125).bold();
    let color_syntax: Style = Colour::Fixed(8).italic();

    for (atom, _errors) in parser {
        match atom.kind {
            AtomKind::Section { name } => {
                printer.print_word(name, color_section, &mut output)?;
            }
            AtomKind::Comment {
                open,
                content,
                close,
            } => {
                printer.print_word(open, color_comment, &mut output)?;
                printer.print_string(
                    &content,
                    open.location.end() + content.len() as isize,
                    color_comment,
                    &mut output,
                )?;
                if let Some(close) = close {
                    printer.print_word(close, color_comment, &mut output)?;
                }
            }
            AtomKind::OpenBlock { head } | AtomKind::CloseBlock { head } => {
                printer.print_word(head, color_syntax, &mut output)?;
            }
            AtomKind::If { head, condition } | AtomKind::ElseIf { head, condition } => {
                printer.print_word(head, color_keyword, &mut output)?;
                printer.print_word(condition, color_const, &mut output)?;
            }
            AtomKind::Else { head }
            | AtomKind::EndIf { head }
            | AtomKind::StartRandom { head }
            | AtomKind::EndRandom { head } => {
                printer.print_word(head, color_keyword, &mut output)?;
            }
            AtomKind::PercentChance { head, chance } => {
                printer.print_word(head, color_keyword, &mut output)?;
                printer.print_word(chance, color_number, &mut output)?;
            }
            AtomKind::Command { name, arguments } => {
                printer.print_word(name, color_attribute, &mut output)?;
                for arg in arguments {
                    let colour = if arg.value.chars().all(|c| c.is_ascii_digit()) {
                        color_number
                    } else {
                        color_const
                    };
                    printer.print_word(arg, colour, &mut output)?;
                }
            }
            _ => {}
        }
    }

    printer.print_whitespace(ByteIndex::from(source.len()), &mut output)?;
    Ok(())
}

pub fn highlight_to(source: &str, output: impl Write) -> Result<()> {
    let file = RMSFile::from_string("highlight.rms", source);

    highlight_atoms_to(source, Parser::new(file.file_id(), source), output)
}

pub fn highlight(source: &str) -> String {
    let mut output = Cursor::new(vec![]);
    highlight_to(source, &mut output).unwrap();
    String::from_utf8(output.into_inner()).unwrap()
}
