use stk_span::{Diagnostic, Span};

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Fn,
    Var,
    Const,
    Return,
    Int,
    Float,
    String,
    Bool,
    If,
    Else,
    While,
    For,
    In,
    Break,
    Continue,
    Match,
    Do,
    Try,
    Catch,
    True,
    False,
    Class,
    Struct,
    IClass,
    New,
    SelfKw,
    Super,
    Pub,
    Priv,
    Prot,
    Async,
    Await,
    Spawn,

    Ident(String),
    IntLit(i64),
    FloatLit(f64),
    StringLit(String),

    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Semicolon,
    Colon,
    ColonColon,
    Eq,
    EqEq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    AndAnd,
    OrOr,
    Bang,
    Question,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Dot,
    DotDot,
    FatArrow,
    At,
    Underscore,

    Eof,
}

pub struct Lexer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Self {
            src,
            bytes: src.as_bytes(),
            pos: 0,
        }
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>, Diagnostic> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token()?;
            let is_eof = tok.kind == TokenKind::Eof;
            tokens.push(tok);
            if is_eof {
                break;
            }
        }
        Ok(tokens)
    }

    fn next_token(&mut self) -> Result<Token, Diagnostic> {
        self.skip_trivia()?;
        let start = self.pos;
        if self.pos >= self.bytes.len() {
            return Ok(Token {
                kind: TokenKind::Eof,
                span: Span::new(start, start),
            });
        }

        let b = self.bytes[self.pos];
        match b {
            b'(' => self.simple(TokenKind::LParen, start),
            b')' => self.simple(TokenKind::RParen, start),
            b'{' => self.simple(TokenKind::LBrace, start),
            b'}' => self.simple(TokenKind::RBrace, start),
            b'[' => self.simple(TokenKind::LBracket, start),
            b']' => self.simple(TokenKind::RBracket, start),
            b',' => self.simple(TokenKind::Comma, start),
            b';' => self.simple(TokenKind::Semicolon, start),
            b'@' => self.simple(TokenKind::At, start),
            b'+' => self.simple(TokenKind::Plus, start),
            b'*' => self.simple(TokenKind::Star, start),
            b'%' => self.simple(TokenKind::Percent, start),
            b'!' => {
                if self.peek_byte(1) == Some(b'=') {
                    self.pos += 2;
                    Ok(Token {
                        kind: TokenKind::Ne,
                        span: Span::new(start, self.pos),
                    })
                } else {
                    self.simple(TokenKind::Bang, start)
                }
            }
            b'?' => self.simple(TokenKind::Question, start),
            b'=' => {
                if self.peek_byte(1) == Some(b'=') {
                    self.pos += 2;
                    Ok(Token {
                        kind: TokenKind::EqEq,
                        span: Span::new(start, self.pos),
                    })
                } else if self.peek_byte(1) == Some(b'>') {
                    self.pos += 2;
                    Ok(Token {
                        kind: TokenKind::FatArrow,
                        span: Span::new(start, self.pos),
                    })
                } else {
                    self.simple(TokenKind::Eq, start)
                }
            }
            b'<' => {
                if self.peek_byte(1) == Some(b'=') {
                    self.pos += 2;
                    Ok(Token {
                        kind: TokenKind::Le,
                        span: Span::new(start, self.pos),
                    })
                } else {
                    self.simple(TokenKind::Lt, start)
                }
            }
            b'>' => {
                if self.peek_byte(1) == Some(b'=') {
                    self.pos += 2;
                    Ok(Token {
                        kind: TokenKind::Ge,
                        span: Span::new(start, self.pos),
                    })
                } else {
                    self.simple(TokenKind::Gt, start)
                }
            }
            b'&' => {
                if self.peek_byte(1) == Some(b'&') {
                    self.pos += 2;
                    Ok(Token {
                        kind: TokenKind::AndAnd,
                        span: Span::new(start, self.pos),
                    })
                } else {
                    Err(Diagnostic::new(
                        "unexpected '&'; did you mean '&&'?",
                        Span::new(start, start + 1),
                    ))
                }
            }
            b'|' => {
                if self.peek_byte(1) == Some(b'|') {
                    self.pos += 2;
                    Ok(Token {
                        kind: TokenKind::OrOr,
                        span: Span::new(start, self.pos),
                    })
                } else {
                    Err(Diagnostic::new(
                        "unexpected '|'; did you mean '||'?",
                        Span::new(start, start + 1),
                    ))
                }
            }
            b':' => {
                if self.peek_byte(1) == Some(b':') {
                    self.pos += 2;
                    Ok(Token {
                        kind: TokenKind::ColonColon,
                        span: Span::new(start, self.pos),
                    })
                } else {
                    self.simple(TokenKind::Colon, start)
                }
            }
            b'-' => self.simple(TokenKind::Minus, start),
            b'/' => self.simple(TokenKind::Slash, start),
            b'.' => {
                if self.peek_byte(1) == Some(b'.') {
                    self.pos += 2;
                    Ok(Token {
                        kind: TokenKind::DotDot,
                        span: Span::new(start, self.pos),
                    })
                } else {
                    self.simple(TokenKind::Dot, start)
                }
            }
            b'"' => self.string_lit(start),
            b'0'..=b'9' => self.int_lit(start),
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.ident_or_kw(start),
            _ => {
                let ch = self.src[self.pos..].chars().next().unwrap_or('?');
                Err(Diagnostic::new(
                    format!("unexpected character '{ch}'"),
                    Span::new(start, start + ch.len_utf8()),
                ))
            }
        }
    }

    fn peek_byte(&self, offset: usize) -> Option<u8> {
        self.bytes.get(self.pos + offset).copied()
    }

    fn simple(&mut self, kind: TokenKind, start: usize) -> Result<Token, Diagnostic> {
        self.pos += 1;
        Ok(Token {
            kind,
            span: Span::new(start, self.pos),
        })
    }

    fn skip_trivia(&mut self) -> Result<(), Diagnostic> {
        loop {
            while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
                self.pos += 1;
            }
            if self.pos + 1 < self.bytes.len()
                && self.bytes[self.pos] == b'/'
                && self.bytes[self.pos + 1] == b'/'
            {
                self.pos += 2;
                while self.pos < self.bytes.len() && self.bytes[self.pos] != b'\n' {
                    self.pos += 1;
                }
                continue;
            }
            if self.pos + 1 < self.bytes.len()
                && self.bytes[self.pos] == b'/'
                && self.bytes[self.pos + 1] == b'*'
            {
                let start = self.pos;
                self.pos += 2;
                loop {
                    if self.pos + 1 >= self.bytes.len() {
                        return Err(Diagnostic::new(
                            "unterminated block comment",
                            Span::new(start, self.pos),
                        ));
                    }
                    if self.bytes[self.pos] == b'*' && self.bytes[self.pos + 1] == b'/' {
                        self.pos += 2;
                        break;
                    }
                    self.pos += 1;
                }
                continue;
            }
            break;
        }
        Ok(())
    }

    fn string_lit(&mut self, start: usize) -> Result<Token, Diagnostic> {
        self.pos += 1;
        let mut value = String::new();
        while self.pos < self.bytes.len() {
            let ch = self.src[self.pos..].chars().next().unwrap();
            let len = ch.len_utf8();
            if ch == '"' {
                self.pos += 1;
                return Ok(Token {
                    kind: TokenKind::StringLit(value),
                    span: Span::new(start, self.pos),
                });
            }
            if ch == '\\' {
                self.pos += 1;
                if self.pos >= self.bytes.len() {
                    break;
                }
                let esc = self.src[self.pos..].chars().next().unwrap();
                let elen = esc.len_utf8();
                value.push(match esc {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    '\\' => '\\',
                    '"' => '"',
                    '$' => '$',
                    other => other,
                });
                self.pos += elen;
                continue;
            }
            value.push(ch);
            self.pos += len;
        }
        Err(Diagnostic::new(
            "unterminated string literal",
            Span::new(start, self.pos),
        ))
    }

    fn int_lit(&mut self, start: usize) -> Result<Token, Diagnostic> {
        while self.pos < self.bytes.len()
            && (self.bytes[self.pos].is_ascii_digit() || self.bytes[self.pos] == b'_')
        {
            self.pos += 1;
        }
        // Float if fractional part follows: `1.0`
        if self.pos < self.bytes.len()
            && self.bytes[self.pos] == b'.'
            && self
                .bytes
                .get(self.pos + 1)
                .is_some_and(|b| b.is_ascii_digit())
        {
            self.pos += 1;
            while self.pos < self.bytes.len()
                && (self.bytes[self.pos].is_ascii_digit() || self.bytes[self.pos] == b'_')
            {
                self.pos += 1;
            }
            let raw = &self.src[start..self.pos];
            let cleaned: String = raw.chars().filter(|c| *c != '_').collect();
            let value = cleaned.parse::<f64>().map_err(|_| {
                Diagnostic::new(
                    format!("invalid float literal '{raw}'"),
                    Span::new(start, self.pos),
                )
            })?;
            return Ok(Token {
                kind: TokenKind::FloatLit(value),
                span: Span::new(start, self.pos),
            });
        }
        let raw = &self.src[start..self.pos];
        let cleaned: String = raw.chars().filter(|c| *c != '_').collect();
        let value = cleaned.parse::<i64>().map_err(|_| {
            Diagnostic::new(
                format!("invalid integer literal '{raw}'"),
                Span::new(start, self.pos),
            )
        })?;
        Ok(Token {
            kind: TokenKind::IntLit(value),
            span: Span::new(start, self.pos),
        })
    }

    fn ident_or_kw(&mut self, start: usize) -> Result<Token, Diagnostic> {
        while self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            if b.is_ascii_alphanumeric() || b == b'_' {
                self.pos += 1;
            } else {
                break;
            }
        }
        let text = &self.src[start..self.pos];
        let kind = match text {
            "fn" => TokenKind::Fn,
            "var" => TokenKind::Var,
            "const" => TokenKind::Const,
            "return" => TokenKind::Return,
            "async" => TokenKind::Async,
            "await" => TokenKind::Await,
            "spawn" => TokenKind::Spawn,
            "int" => TokenKind::Int,
            "float" => TokenKind::Float,
            "string" => TokenKind::String,
            "bool" => TokenKind::Bool,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "match" => TokenKind::Match,
            "do" => TokenKind::Do,
            "try" => TokenKind::Try,
            "catch" => TokenKind::Catch,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "class" => TokenKind::Class,
            "struct" => TokenKind::Struct,
            "iclass" => TokenKind::IClass,
            "new" => TokenKind::New,
            "self" => TokenKind::SelfKw,
            "super" => TokenKind::Super,
            "pub" => TokenKind::Pub,
            "priv" => TokenKind::Priv,
            "prot" => TokenKind::Prot,
            "import" => TokenKind::Ident("import".into()),
            "_" => TokenKind::Underscore,
            "let" => {
                return Err(Diagnostic::new(
                    "'let' is not a keyword in Steampunk; use 'var'",
                    Span::new(start, self.pos),
                ));
            }
            other => TokenKind::Ident(other.to_string()),
        };
        Ok(Token {
            kind,
            span: Span::new(start, self.pos),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexes_control_tokens() {
        let tokens = Lexer::new("if true && x == 1 { } match _ =>").tokenize().unwrap();
        assert!(matches!(tokens[0].kind, TokenKind::If));
        assert!(matches!(tokens[1].kind, TokenKind::True));
        assert!(matches!(tokens[2].kind, TokenKind::AndAnd));
    }
}
