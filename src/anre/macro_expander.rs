// Copyright (c) 2026 Hemashushu <hippospark@gmail.com>, All rights reserved.
//
// This Source Code Form is subject to the terms of
// the Mozilla Public License version 2.0 and additional exceptions.
// For more details, see the LICENSE, LICENSE.additional, and CONTRIBUTING files.

use crate::{error::AnreError, peekable_iter::PeekableIter, range::Range};

use super::token::{Token, TokenWithRange};

/// Expands macros in the token stream by replacing definition identifiers
/// with their corresponding replacement tokens.
pub fn expand(tokens: Vec<TokenWithRange>) -> Result<Vec<TokenWithRange>, AnreError> {
    let (program_tokens, definitions) = extract_definitions(tokens)?;
    let expand_tokens = replace_identifiers(program_tokens, definitions);

    Ok(expand_tokens)
}

/// Extracts macro definitions from the token stream.
///
/// A macro definition has the form: `define name (body)`,
/// where `name` is an identifier and `body` is a sequence of tokens.
fn extract_definitions(
    mut tokens: Vec<TokenWithRange>,
) -> Result<(Vec<TokenWithRange>, Vec<Definition>), AnreError> {
    let mut definitions: Vec<Definition> = vec![];
    loop {
        let define_keyword_index_opt = tokens.iter().position(|token_with_range| {
            matches!(token_with_range, TokenWithRange {
                token: Token::Keyword(keyword),
                ..  } if keyword == "define" )
        });

        if define_keyword_index_opt.is_none() {
            break;
        }

        let define_keyword_index = define_keyword_index_opt.unwrap();
        let mut parenthesis_depth: usize = 0;

        let mut end_index_opt: Option<usize> = None;
        let mut idx = define_keyword_index + 1;

        // determine the definition statement range by finding
        // the matching parenthesis pair after the "define" keyword.
        while idx < tokens.len() {
            match tokens[idx].token {
                Token::ParenthesisOpen => {
                    // found '('
                    parenthesis_depth += 1;
                }
                Token::ParenthesisClose => {
                    // found ')'
                    if parenthesis_depth == 0 {
                        return Err(AnreError::MessageWithRange(
                            "Unexpected ')' without matching '('.".to_owned(),
                            tokens[idx].range,
                        ));
                    } else if parenthesis_depth == 1 {
                        // found the matching parenthesis pair for the current definition statement
                        end_index_opt = Some(idx);
                        break;
                    } else {
                        parenthesis_depth -= 1;
                    }
                }
                _ => {
                    // tokens inside the definition body, do nothing
                }
            }

            idx += 1;
        }

        // extract one definition
        if let Some(end_index) = end_index_opt {
            // Remove the definition statement tokens from the original token stream
            let definition_tokens: Vec<TokenWithRange> = tokens
                .drain(define_keyword_index..(end_index + 1))
                .collect();

            // Extract the definition from the definition statement tokens, and store it in the definitions list.
            let mut token_iter = definition_tokens.into_iter();
            let mut peekable_token_iter = PeekableIter::new(&mut token_iter);
            let mut extractor = DefinitionExtractor::new(&mut peekable_token_iter);
            let definition = extractor.extract()?;
            definitions.push(definition);
        } else {
            return Err(AnreError::UnexpectedEndOfDocument(
                "Incomplete definition statement.".to_owned(),
            ));
        }
    }

    Ok((tokens, definitions))
}

/// Replaces identifiers in the token stream with their corresponding macro definitions.
fn replace_identifiers(
    mut program_tokens: Vec<TokenWithRange>,
    mut definitions: Vec<Definition>,
) -> Vec<TokenWithRange> {
    // Reverse the definitions list to ensure that definitions are replaced in the correct order,
    // i.e., if definition A references definition B, then B should be replaced before A.
    definitions.reverse();

    while let Some(definition) = definitions.pop() {
        // Expand the current definition in all the remaining definitions
        for idx in (0..definitions.len()).rev() {
            find_and_replace_identifiers(
                &mut definitions[idx].tokens,
                &definition.name,
                &definition.tokens,
            );
        }

        // Expand the current definition in the program tokens
        find_and_replace_identifiers(&mut program_tokens, &definition.name, &definition.tokens);
    }

    program_tokens
}

fn find_and_replace_identifiers(
    source_tokens: &mut Vec<TokenWithRange>,
    find_id: &str,
    replace_with: &[TokenWithRange],
) {
    for idx in (0..source_tokens.len()).rev() {
        if let Token::Identifier(id) = &source_tokens[idx].token {
            if id == find_id {
                // Remove the identifier token, and insert the replacement tokens
                source_tokens.splice(idx..(idx + 1), replace_with.iter().cloned());
            }
        }
    }
}

#[derive(Debug, PartialEq)]
struct Definition {
    name: String,
    tokens: Vec<TokenWithRange>,
}

pub struct DefinitionExtractor<'a> {
    upstream: &'a mut PeekableIter<'a, TokenWithRange>,

    /// The range of the last consumed token by `next_token` or `next_token_with_range`.
    last_range: Range,
}

impl<'a> DefinitionExtractor<'a> {
    fn new(upstream: &'a mut PeekableIter<'a, TokenWithRange>) -> Self {
        Self {
            upstream,
            last_range: Range::default(),
        }
    }
    fn next_token(&mut self) -> Option<Token> {
        match self.next_token_with_range() {
            Some(TokenWithRange { token, range }) => {
                self.last_range = range;
                Some(token)
            }
            None => None,
        }
    }

    fn next_token_with_range(&mut self) -> Option<TokenWithRange> {
        match self.upstream.next() {
            Some(token_with_range) => {
                self.last_range = token_with_range.range;
                Some(token_with_range)
            }
            None => None,
        }
    }

    fn peek_token(&self, offset: usize) -> Option<&Token> {
        match self.upstream.peek(offset) {
            Some(TokenWithRange { token, .. }) => Some(token),
            None => None,
        }
    }

    fn peek_token_with_range(&self, offset: usize) -> Option<&TokenWithRange> {
        self.upstream.peek(offset)
    }

    fn peek_range(&self, offset: usize) -> Option<&Range> {
        match self.upstream.peek(offset) {
            Some(TokenWithRange { range, .. }) => Some(range),
            None => None,
        }
    }

    // Peek the next token and check if it equals to the expected token,
    // return false if not equals or no more token,
    // error if lexing error occurs during peeking.
    fn peek_token_and_equals(&self, offset: usize, expected_token: &Token) -> bool {
        matches!(
            self.peek_token(offset),
            Some(token) if token == expected_token)
    }

    fn consume_identifier(&mut self) -> Result<String, AnreError> {
        match self.next_token() {
            Some(Token::Identifier(id)) => Ok(id),
            Some(_) => Err(AnreError::MessageWithPosition(
                "Expect an identifier.".to_owned(),
                self.last_range.start,
            )),
            None => Err(AnreError::UnexpectedEndOfDocument(
                "Expect an identifier.".to_owned(),
            )),
        }
    }

    fn extract(&mut self) -> Result<Definition, AnreError> {
        // ```diagram
        // "define" name "(" body ")" ?
        // --------      ---      ---
        // ^             ^        ^__ validated
        // |             |__ validated
        // | current, validated
        // ```

        self.next_token(); // consume "define"
        let name = self.consume_identifier()?;
        let tokens: Vec<TokenWithRange> = self.upstream.collect();

        let definition = Definition { name, tokens };

        Ok(definition)
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::{
        anre::{
            lexer::lex_from_str,
            token::{Token, TokenWithRange},
        },
        error::AnreError,
    };

    use super::expand;

    fn lex_and_expand_from_str(s: &str) -> Result<Vec<TokenWithRange>, AnreError> {
        let tokens = lex_from_str(s)?;
        let expanded_tokens = expand(tokens)?;
        Ok(expanded_tokens)
    }

    fn lex_and_expand_from_str_without_location(s: &str) -> Result<Vec<Token>, AnreError> {
        let tokens = lex_and_expand_from_str(s)?
            .into_iter()
            .map(|e| e.token)
            .collect::<Vec<Token>>();
        Ok(tokens)
    }

    #[test]
    fn test_macro_expand() {
        assert_eq!(
            lex_and_expand_from_str_without_location(
                r#"
            define MACRO_A ('a')
            ('x', MACRO_A, 'y')
            "#,
            )
            .unwrap(),
            vec![
                Token::ParenthesisOpen,
                Token::Char('x'),
                // MACRO_A
                Token::ParenthesisOpen,
                Token::Char('a'),
                Token::ParenthesisClose,
                //
                Token::Char('y'),
                Token::ParenthesisClose
            ]
        );

        assert_eq!(
            lex_and_expand_from_str_without_location(
                r#"
            define MACRO_A ('a')
            define MACRO_B (MACRO_A+)
            (MACRO_A, MACRO_B)
            "#,
            )
            .unwrap(),
            vec![
                Token::ParenthesisOpen,
                // MACRO_A
                Token::ParenthesisOpen,
                Token::Char('a'),
                Token::ParenthesisClose,
                // MACRO_B
                Token::ParenthesisOpen,
                Token::ParenthesisOpen,  // MACRO_A
                Token::Char('a'),        // MACRO_A
                Token::ParenthesisClose, // MACRO_A
                Token::Plus,
                Token::ParenthesisClose,
                //
                Token::ParenthesisClose
            ]
        );

        assert_eq!(
            lex_and_expand_from_str_without_location(
                r#"
            define MACRO_A (char_word)
            #MACRO_A as 🔑
            "#,
            )
            .unwrap(),
            vec![
                Token::Hash,
                Token::ParenthesisOpen,
                Token::Identifier("char_word".to_owned()),
                Token::ParenthesisClose,
                Token::Keyword("as".to_owned()),
                Token::Identifier("🔑".to_owned()),
            ]
        );
    }
}
