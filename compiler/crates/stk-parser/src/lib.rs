use stk_ast::*;
use stk_lexer::{Lexer, Token, TokenKind};
use stk_span::{Diagnostic, Span};

pub fn parse(source: &str) -> Result<Program, Diagnostic> {
    let tokens = Lexer::new(source).tokenize()?;
    Parser::new(tokens).parse_program()
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn bump(&mut self) -> Token {
        let t = self.tokens[self.pos.min(self.tokens.len() - 1)].clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn expect(&mut self, kind: TokenKind) -> Result<Span, Diagnostic> {
        let tok = self.peek().clone();
        if matches_kind(&tok.kind, &kind) {
            let span = tok.span;
            self.bump();
            Ok(span)
        } else {
            Err(Diagnostic::new(
                format!("expected {:?}, found {:?}", kind, tok.kind),
                tok.span,
            ))
        }
    }

    fn expect_ident(&mut self) -> Result<(String, Span), Diagnostic> {
        let tok = self.peek().clone();
        match tok.kind {
            TokenKind::Ident(name) => {
                self.bump();
                Ok((name, tok.span))
            }
            _ => Err(Diagnostic::new(
                format!("expected identifier, found {:?}", tok.kind),
                tok.span,
            )),
        }
    }

    /// Namespaces under `std.` may share a name with a type keyword (`std.string`).
    fn expect_std_member(&mut self) -> Result<(String, Span), Diagnostic> {
        let tok = self.peek().clone();
        let name = match tok.kind {
            TokenKind::Ident(ref name) => name.clone(),
            TokenKind::String => "string".to_string(),
            TokenKind::Int => "int".to_string(),
            TokenKind::Float => "float".to_string(),
            TokenKind::Bool => "bool".to_string(),
            _ => {
                return Err(Diagnostic::new(
                    format!("expected std member, found {:?}", tok.kind),
                    tok.span,
                ));
            }
        };
        self.bump();
        Ok((name, tok.span))
    }

    /// Method names may be `new` (keyword) or a normal identifier.
    fn expect_method_name(&mut self) -> Result<(String, Span), Diagnostic> {
        let tok = self.peek().clone();
        match tok.kind {
            TokenKind::Ident(name) => {
                self.bump();
                Ok((name, tok.span))
            }
            TokenKind::New => {
                self.bump();
                Ok(("new".into(), tok.span))
            }
            _ => Err(Diagnostic::new(
                format!("expected method name, found {:?}", tok.kind),
                tok.span,
            )),
        }
    }

    fn parse_program(&mut self) -> Result<Program, Diagnostic> {
        let mut imports = Vec::new();
        let mut constants = Vec::new();
        let mut functions = Vec::new();
        let mut classes = Vec::new();
        let mut iclasses = Vec::new();

        while !matches!(self.peek().kind, TokenKind::Eof) {
            match &self.peek().kind {
                TokenKind::At => imports.push(self.parse_import()?),
                TokenKind::Const => constants.push(self.parse_const_decl()?),
                TokenKind::Fn => functions.push(self.parse_function(false, false)?),
                TokenKind::Async => {
                    self.bump();
                    functions.push(self.parse_function(false, true)?);
                }
                TokenKind::Pub => {
                    let pub_span = self.bump().span;
                    match &self.peek().kind {
                        TokenKind::Class | TokenKind::Struct => {
                            classes.push(self.parse_class(true, pub_span)?)
                        }
                        TokenKind::IClass => {
                            iclasses.push(self.parse_iclass(true, pub_span)?)
                        }
                        TokenKind::Fn => {
                            functions.push(self.parse_function(true, false)?)
                        }
                        TokenKind::Async => {
                            self.bump();
                            functions.push(self.parse_function(true, true)?)
                        }
                        _ => {
                            return Err(Diagnostic::new(
                                "expected fn, async, class, or iclass after pub",
                                self.peek().span,
                            ));
                        }
                    }
                }
                TokenKind::Class | TokenKind::Struct => {
                    let span = self.peek().span;
                    classes.push(self.parse_class(false, span)?)
                }
                TokenKind::IClass => {
                    let span = self.peek().span;
                    iclasses.push(self.parse_iclass(false, span)?)
                }
                _ => {
                    return Err(Diagnostic::new(
                        format!("unexpected token at top level: {:?}", self.peek().kind),
                        self.peek().span,
                    ));
                }
            }
        }

        Ok(Program {
            imports,
            constants,
            functions,
            classes,
            iclasses,
        })
    }

    fn parse_const_decl(&mut self) -> Result<ConstDecl, Diagnostic> {
        let start = self.expect(TokenKind::Const)?;
        let (name, _) = self.expect_ident()?;
        self.expect(TokenKind::Eq)?;
        let value = self.parse_literal_expr()?;
        let end = value.span().end;
        Ok(ConstDecl {
            name,
            value,
            span: Span::new(start.start, end),
            module: String::new(),
        })
    }

    fn parse_import(&mut self) -> Result<Import, Diagnostic> {
        let start = self.expect(TokenKind::At)?;
        let (ident, _) = self.expect_ident()?;
        if ident != "import" {
            return Err(Diagnostic::new(
                "expected 'import' after '@'",
                self.peek().span,
            ));
        }
        let tok = self.peek().clone();
        let path = match tok.kind {
            TokenKind::StringLit(p) => {
                self.bump();
                p
            }
            _ => {
                return Err(Diagnostic::new(
                    "expected string path after @import",
                    tok.span,
                ));
            }
        };
        Ok(Import {
            path,
            span: Span::new(start.start, tok.span.end),
        })
    }

    fn parse_visibility(&mut self) -> Result<Visibility, Diagnostic> {
        match self.peek().kind {
            TokenKind::Pub => {
                self.bump();
                Ok(Visibility::Pub)
            }
            TokenKind::Priv => {
                self.bump();
                Ok(Visibility::Priv)
            }
            TokenKind::Prot => {
                self.bump();
                Ok(Visibility::Prot)
            }
            _ => Err(Diagnostic::new(
                "class members require pub, priv, or prot",
                self.peek().span,
            )),
        }
    }

    fn parse_class(&mut self, is_pub: bool, start: Span) -> Result<ClassDecl, Diagnostic> {
        match &self.peek().kind {
            TokenKind::Class | TokenKind::Struct => {
                self.bump();
            }
            _ => {
                return Err(Diagnostic::new(
                    "expected class or struct",
                    self.peek().span,
                ));
            }
        }
        let (name, _) = self.expect_ident()?;
        let type_params = self.parse_type_params()?;

        let mut bases = Vec::new();
        if matches!(self.peek().kind, TokenKind::ColonColon) {
            self.bump();
            loop {
                let (b, _) = self.expect_ident()?;
                bases.push(b);
                if matches!(self.peek().kind, TokenKind::Comma) {
                    self.bump();
                    continue;
                }
                break;
            }
        }

        let mut interfaces = Vec::new();
        if matches!(self.peek().kind, TokenKind::Colon) {
            self.bump();
            loop {
                let (iname, _) = self.expect_ident()?;
                interfaces.push(iname);
                if matches!(self.peek().kind, TokenKind::Comma) {
                    self.bump();
                    continue;
                }
                break;
            }
        }

        self.expect(TokenKind::LBrace)?;
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        while !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
            let attrs = self.parse_attributes()?;
            if !attrs.is_empty() && matches!(self.peek().kind, TokenKind::Fn) {
                return Err(Diagnostic::new(
                    "decorators are not allowed on methods",
                    attrs[0].span,
                ));
            }
            // Decorators may appear before visibility
            if matches!(self.peek().kind, TokenKind::At) {
                // already consumed by parse_attributes unless empty path
            }
            let vis = self.parse_visibility()?;
            if matches!(self.peek().kind, TokenKind::Var) {
                fields.push(self.parse_field(vis, attrs)?);
            } else if matches!(self.peek().kind, TokenKind::Fn) {
                if !attrs.is_empty() {
                    return Err(Diagnostic::new(
                        "decorators are not allowed on methods",
                        attrs[0].span,
                    ));
                }
                methods.push(self.parse_method(vis)?);
            } else {
                return Err(Diagnostic::new(
                    "expected field (var) or method (fn) in class body",
                    self.peek().span,
                ));
            }
        }
        let end = self.expect(TokenKind::RBrace)?;
        Ok(ClassDecl {
            is_pub,
            name,
            type_params,
            bases,
            interfaces,
            fields,
            methods,
            span: Span::new(start.start, end.end),
            module: String::new(),
        })
    }

    fn parse_attributes(&mut self) -> Result<Vec<Attribute>, Diagnostic> {
        let mut attrs = Vec::new();
        while matches!(self.peek().kind, TokenKind::At) {
            let start = self.bump().span;
            let (name, name_span) = self.expect_ident()?;
            if name == "import" {
                return Err(Diagnostic::new(
                    "@import is only valid at file top-level",
                    start,
                ));
            }
            let mut args = Vec::new();
            if matches!(self.peek().kind, TokenKind::LParen) {
                self.bump();
                if !matches!(self.peek().kind, TokenKind::RParen) {
                    loop {
                        args.push(self.parse_literal_expr()?);
                        if matches!(self.peek().kind, TokenKind::Comma) {
                            self.bump();
                            continue;
                        }
                        break;
                    }
                }
                let end = self.expect(TokenKind::RParen)?;
                attrs.push(Attribute {
                    name,
                    args,
                    span: Span::new(start.start, end.end),
                });
            } else {
                attrs.push(Attribute {
                    name,
                    args,
                    span: Span::new(start.start, name_span.end),
                });
            }
        }
        Ok(attrs)
    }

    fn parse_field(&mut self, vis: Visibility, attrs: Vec<Attribute>) -> Result<FieldDecl, Diagnostic> {
        let start = self.expect(TokenKind::Var)?;
        let span_start = attrs
            .first()
            .map(|a| a.span.start)
            .unwrap_or(start.start);
        let (name, _) = self.expect_ident()?;
        let ty = self.parse_type()?;
        let mut default = None;
        let mut accessors = None;
        let mut end = start.end;

        if matches!(self.peek().kind, TokenKind::LBrace) {
            let acc = self.parse_prop_accessors()?;
            end = acc.1;
            accessors = Some(acc.0);
        } else if matches!(self.peek().kind, TokenKind::Eq) {
            self.bump();
            let expr = self.parse_literal_expr()?;
            end = expr.span().end;
            default = Some(expr);
        }

        if accessors.is_some() && default.is_some() {
            return Err(Diagnostic::new(
                "property with get/set cannot have a field default",
                start,
            ));
        }

        Ok(FieldDecl {
            attrs,
            vis,
            name,
            ty,
            default,
            accessors,
            span: Span::new(span_start, end),
        })
    }

    fn parse_prop_accessors(&mut self) -> Result<(PropAccessors, usize), Diagnostic> {
        self.expect(TokenKind::LBrace)?;
        let mut getter = None;
        let mut setter = None;
        while !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
            let (kw, kw_span) = self.expect_ident()?;
            match kw.as_str() {
                "get" => {
                    if getter.is_some() {
                        return Err(Diagnostic::new("duplicate get", kw_span));
                    }
                    getter = Some(self.parse_block()?);
                }
                "set" => {
                    if setter.is_some() {
                        return Err(Diagnostic::new("duplicate set", kw_span));
                    }
                    self.expect(TokenKind::LParen)?;
                    let param = self.parse_param()?;
                    if param.default.is_some() {
                        return Err(Diagnostic::new(
                            "setter parameter cannot have a default",
                            param.span,
                        ));
                    }
                    self.expect(TokenKind::RParen)?;
                    let body = self.parse_block()?;
                    setter = Some(SetterDecl { param, body });
                }
                _ => {
                    return Err(Diagnostic::new(
                        "expected get or set in property body",
                        kw_span,
                    ));
                }
            }
        }
        let end = self.expect(TokenKind::RBrace)?;
        if getter.is_none() && setter.is_none() {
            return Err(Diagnostic::new(
                "property needs at least get or set",
                end,
            ));
        }
        Ok((PropAccessors { getter, setter }, end.end))
    }

    /// Defaults accept only literals (int/string/bool).
    fn parse_literal_expr(&mut self) -> Result<Expr, Diagnostic> {
        let tok = self.peek().clone();
        match tok.kind {
            TokenKind::IntLit(value) => {
                self.bump();
                Ok(Expr::IntLit {
                    value,
                    span: tok.span,
                })
            }
            TokenKind::FloatLit(value) => {
                self.bump();
                Ok(Expr::FloatLit {
                    value,
                    span: tok.span,
                })
            }
            TokenKind::StringLit(value) => {
                self.bump();
                Ok(Expr::StringLit {
                    value,
                    span: tok.span,
                })
            }
            TokenKind::True => {
                self.bump();
                Ok(Expr::BoolLit {
                    value: true,
                    span: tok.span,
                })
            }
            TokenKind::False => {
                self.bump();
                Ok(Expr::BoolLit {
                    value: false,
                    span: tok.span,
                })
            }
            TokenKind::Minus => {
                self.bump();
                match self.peek().kind.clone() {
                    TokenKind::IntLit(value) => {
                        let span = self.bump().span;
                        Ok(Expr::IntLit {
                            value: -value,
                            span: Span::new(tok.span.start, span.end),
                        })
                    }
                    TokenKind::FloatLit(value) => {
                        let span = self.bump().span;
                        Ok(Expr::FloatLit {
                            value: -value,
                            span: Span::new(tok.span.start, span.end),
                        })
                    }
                    _ => Err(Diagnostic::new(
                        "default value must be a literal",
                        self.peek().span,
                    )),
                }
            }
            _ => Err(Diagnostic::new(
                "default value must be a literal (int, float, string, or bool)",
                tok.span,
            )),
        }
    }

    fn parse_method(&mut self, vis: Visibility) -> Result<MethodDecl, Diagnostic> {
        let start = self.expect(TokenKind::Fn)?;
        let (name, _) = self.expect_method_name()?;
        self.expect(TokenKind::LParen)?;
        let mut params = Vec::new();
        if !matches!(self.peek().kind, TokenKind::RParen) {
            loop {
                params.push(self.parse_param()?);
                if matches!(self.peek().kind, TokenKind::Comma) {
                    self.bump();
                    continue;
                }
                break;
            }
        }
        self.expect(TokenKind::RParen)?;
        let return_ty = if matches!(
            self.peek().kind,
            TokenKind::Int
                | TokenKind::Float
                | TokenKind::String
                | TokenKind::Bool
                | TokenKind::Ident(_)
                | TokenKind::LBrace
        ) {
            if matches!(self.peek().kind, TokenKind::LBrace) {
                None
            } else {
                Some(self.parse_type()?)
            }
        } else {
            None
        };
        let body = self.parse_block()?;
        let end = body.span.end;
        Ok(MethodDecl {
            vis,
            name,
            params,
            return_ty,
            body,
            span: Span::new(start.start, end),
        })
    }

    fn parse_iclass(&mut self, is_pub: bool, start: Span) -> Result<IClassDecl, Diagnostic> {
        self.expect(TokenKind::IClass)?;
        let (name, _) = self.expect_ident()?;
        self.expect(TokenKind::LBrace)?;
        let mut methods = Vec::new();
        while !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
            let (mname, mspan) = self.expect_ident()?;
            self.expect(TokenKind::LParen)?;
            let mut params = Vec::new();
            if !matches!(self.peek().kind, TokenKind::RParen) {
                loop {
                    params.push(self.parse_param()?);
                    if matches!(self.peek().kind, TokenKind::Comma) {
                        self.bump();
                        continue;
                    }
                    break;
                }
            }
            self.expect(TokenKind::RParen)?;
            let return_ty = if matches!(
                self.peek().kind,
                TokenKind::Int | TokenKind::Float | TokenKind::String | TokenKind::Bool | TokenKind::Ident(_)
            ) {
                Some(self.parse_type()?)
            } else {
                None
            };
            methods.push(IClassMethod {
                name: mname,
                params,
                return_ty,
                span: mspan,
            });
        }
        let end = self.expect(TokenKind::RBrace)?;
        Ok(IClassDecl {
            is_pub,
            name,
            methods,
            span: Span::new(start.start, end.end),
            module: String::new(),
        })
    }

    fn parse_function(&mut self, is_pub: bool, is_async: bool) -> Result<Function, Diagnostic> {
        let start = self.expect(TokenKind::Fn)?;
        let (name, _) = self.expect_ident()?;
        let type_params = self.parse_type_params()?;
        self.expect(TokenKind::LParen)?;
        let mut params = Vec::new();
        if !matches!(self.peek().kind, TokenKind::RParen) {
            loop {
                params.push(self.parse_param()?);
                if matches!(self.peek().kind, TokenKind::Comma) {
                    self.bump();
                    continue;
                }
                break;
            }
        }
        self.expect(TokenKind::RParen)?;

        let return_ty = if matches!(
            self.peek().kind,
            TokenKind::Int
                | TokenKind::Float
                | TokenKind::String
                | TokenKind::Bool
                | TokenKind::Ident(_)
                | TokenKind::LBracket
        ) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let body = self.parse_block()?;
        let end = body.span.end;
        Ok(Function {
            is_pub,
            is_async,
            name,
            type_params,
            params,
            return_ty,
            body,
            span: Span::new(start.start, end),
            module: String::new(),
        })
    }

    /// `<T, U>` after a name; empty if no `<`.
    fn parse_type_params(&mut self) -> Result<Vec<String>, Diagnostic> {
        if !matches!(self.peek().kind, TokenKind::Lt) {
            return Ok(Vec::new());
        }
        self.bump();
        let mut params = Vec::new();
        loop {
            let (name, span) = self.expect_ident()?;
            if params.iter().any(|p| p == &name) {
                return Err(Diagnostic::new(
                    format!("duplicate type parameter '{name}'"),
                    span,
                ));
            }
            params.push(name);
            if matches!(self.peek().kind, TokenKind::Comma) {
                self.bump();
                continue;
            }
            break;
        }
        self.expect(TokenKind::Gt)?;
        Ok(params)
    }

    /// Expression form: `fn(params) Ret { body }`
    fn parse_closure_expr(&mut self) -> Result<Expr, Diagnostic> {
        let start = self.expect(TokenKind::Fn)?;
        self.expect(TokenKind::LParen)?;
        let mut params = Vec::new();
        if !matches!(self.peek().kind, TokenKind::RParen) {
            loop {
                params.push(self.parse_param()?);
                if matches!(self.peek().kind, TokenKind::Comma) {
                    self.bump();
                    continue;
                }
                break;
            }
        }
        self.expect(TokenKind::RParen)?;

        let return_ty = if matches!(
            self.peek().kind,
            TokenKind::Int
                | TokenKind::Float
                | TokenKind::String
                | TokenKind::Bool
                | TokenKind::Ident(_)
                | TokenKind::LBracket
        ) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let body = self.parse_block()?;
        Ok(Expr::Closure {
            params,
            return_ty,
            span: Span::new(start.start, body.span.end),
            body,
        })
    }

    fn parse_param(&mut self) -> Result<Param, Diagnostic> {
        let ty = self.parse_type()?;
        let (name, name_span) = self.expect_ident()?;
        let mut default = None;
        let mut end = name_span.end;
        if matches!(self.peek().kind, TokenKind::Eq) {
            self.bump();
            let expr = self.parse_literal_expr()?;
            end = expr.span().end;
            default = Some(expr);
        }
        Ok(Param {
            ty,
            name,
            default,
            span: Span::new(name_span.start, end),
        })
    }

    fn parse_type(&mut self) -> Result<TypeName, Diagnostic> {
        let tok = self.peek().clone();
        match tok.kind {
            TokenKind::Int => {
                self.bump();
                Ok(TypeName::Int)
            }
            TokenKind::Float => {
                self.bump();
                Ok(TypeName::Float)
            }
            TokenKind::String => {
                self.bump();
                Ok(TypeName::String)
            }
            TokenKind::Bool => {
                self.bump();
                Ok(TypeName::Bool)
            }
            TokenKind::LBracket => {
                self.bump();
                let elem = self.parse_type()?;
                self.expect(TokenKind::Semicolon)?;
                let TokenKind::IntLit(len) = self.peek().kind.clone() else {
                    return Err(Diagnostic::new(
                        "expected array length literal",
                        self.peek().span,
                    ));
                };
                self.bump();
                self.expect(TokenKind::RBracket)?;
                Ok(TypeName::Array {
                    elem: Box::new(elem),
                    len,
                })
            }
            TokenKind::Ident(ref s) if s == "void" => {
                self.bump();
                Ok(TypeName::Void)
            }
            TokenKind::Ident(ref s) if s == "Future" => {
                self.bump();
                self.expect(TokenKind::Lt)?;
                let inner = if matches!(self.peek().kind, TokenKind::Ident(ref x) if x == "void")
                {
                    self.bump();
                    TypeName::Void
                } else {
                    self.parse_type()?
                };
                self.expect(TokenKind::Gt)?;
                Ok(TypeName::Future(Box::new(inner)))
            }
            TokenKind::Ident(ref s) if s == "Option" => {
                self.bump();
                self.expect(TokenKind::Lt)?;
                let inner_span = self.peek().span;
                let inner = self.parse_type()?;
                self.expect(TokenKind::Gt)?;
                if matches!(inner, TypeName::Void) {
                    return Err(Diagnostic::new(
                        "Option type cannot be void",
                        inner_span,
                    ));
                }
                Ok(TypeName::Option(Box::new(inner)))
            }
            TokenKind::Ident(ref s) if s == "List" => {
                self.bump();
                self.expect(TokenKind::Lt)?;
                let elem_span = self.peek().span;
                let elem = self.parse_type()?;
                self.expect(TokenKind::Gt)?;
                if matches!(elem, TypeName::Void) {
                    return Err(Diagnostic::new(
                        "List element type cannot be void",
                        elem_span,
                    ));
                }
                Ok(TypeName::List(Box::new(elem)))
            }
            TokenKind::Ident(ref s) if s == "std" => {
                self.bump();
                self.expect(TokenKind::Dot)?;
                let (kind, kind_span) = self.expect_ident()?;
                match kind.as_str() {
                    "sync" => {
                        self.expect(TokenKind::Dot)?;
                        let (sync_kind, sync_span) = self.expect_ident()?;
                        match sync_kind.as_str() {
                            "Channel" => {
                                self.expect(TokenKind::Lt)?;
                                let elem_span = self.peek().span;
                                let elem = self.parse_type()?;
                                if matches!(elem, TypeName::Void) {
                                    return Err(Diagnostic::new(
                                        "Channel element type cannot be void",
                                        elem_span,
                                    ));
                                }
                                self.expect(TokenKind::Gt)?;
                                Ok(TypeName::Channel(Box::new(elem)))
                            }
                            "WaitGroup" => Ok(TypeName::WaitGroup),
                            "Mutex" => {
                                self.expect(TokenKind::Lt)?;
                                let elem_span = self.peek().span;
                                let elem = self.parse_type()?;
                                if matches!(elem, TypeName::Void) {
                                    return Err(Diagnostic::new(
                                        "Mutex element type cannot be void",
                                        elem_span,
                                    ));
                                }
                                self.expect(TokenKind::Gt)?;
                                Ok(TypeName::Mutex(Box::new(elem)))
                            }
                            "RwLock" => {
                                self.expect(TokenKind::Lt)?;
                                let elem_span = self.peek().span;
                                let elem = self.parse_type()?;
                                if matches!(elem, TypeName::Void) {
                                    return Err(Diagnostic::new(
                                        "RwLock element type cannot be void",
                                        elem_span,
                                    ));
                                }
                                self.expect(TokenKind::Gt)?;
                                Ok(TypeName::RwLock(Box::new(elem)))
                            }
                            _ => Err(Diagnostic::new(
                                format!("unknown std.sync type '{sync_kind}'"),
                                sync_span,
                            )),
                        }
                    }
                    "Result" => {
                        self.expect(TokenKind::Lt)?;
                        let ok_span = self.peek().span;
                        let ok = self.parse_type()?;
                        self.expect(TokenKind::Comma)?;
                        let err_span = self.peek().span;
                        let err = self.parse_type()?;
                        self.expect(TokenKind::Gt)?;
                        if matches!(ok, TypeName::Void) {
                            return Err(Diagnostic::new(
                                "Result Ok type cannot be void",
                                ok_span,
                            ));
                        }
                        if matches!(err, TypeName::Void) {
                            return Err(Diagnostic::new(
                                "Result Err type cannot be void",
                                err_span,
                            ));
                        }
                        Ok(TypeName::Result {
                            ok: Box::new(ok),
                            err: Box::new(err),
                        })
                    }
                    "Option" => {
                        self.expect(TokenKind::Lt)?;
                        let inner_span = self.peek().span;
                        let inner = self.parse_type()?;
                        self.expect(TokenKind::Gt)?;
                        if matches!(inner, TypeName::Void) {
                            return Err(Diagnostic::new(
                                "Option type cannot be void",
                                inner_span,
                            ));
                        }
                        Ok(TypeName::Option(Box::new(inner)))
                    }
                    "List" => {
                        self.expect(TokenKind::Lt)?;
                        let elem_span = self.peek().span;
                        let elem = self.parse_type()?;
                        self.expect(TokenKind::Gt)?;
                        if matches!(elem, TypeName::Void) {
                            return Err(Diagnostic::new(
                                "List element type cannot be void",
                                elem_span,
                            ));
                        }
                        Ok(TypeName::List(Box::new(elem)))
                    }
                    _ => Err(Diagnostic::new(
                        format!("unknown std type '{kind}'"),
                        kind_span,
                    )),
                }
            }
            TokenKind::Ident(name) => {
                self.bump();
                Ok(TypeName::Class(name))
            }
            _ => Err(Diagnostic::new(
                format!("expected type, found {:?}", tok.kind),
                tok.span,
            )),
        }
    }

    fn parse_block(&mut self) -> Result<Block, Diagnostic> {
        let start = self.expect(TokenKind::LBrace)?;
        let mut stmts = Vec::new();
        while !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
            stmts.push(self.parse_stmt()?);
        }
        let end = self.expect(TokenKind::RBrace)?;
        Ok(Block {
            stmts,
            span: Span::new(start.start, end.end),
        })
    }

    fn parse_stmt(&mut self) -> Result<Stmt, Diagnostic> {
        match &self.peek().kind {
            TokenKind::Var => self.parse_var_decl(),
            TokenKind::Const => {
                let c = self.parse_const_decl()?;
                Ok(Stmt::ConstDecl {
                    name: c.name,
                    value: c.value,
                    span: c.span,
                })
            }
            TokenKind::Spawn => {
                let start = self.bump().span;
                let body = if matches!(self.peek().kind, TokenKind::LBrace) {
                    SpawnBody::Block(self.parse_block()?)
                } else {
                    SpawnBody::Expr(self.parse_expr()?)
                };
                let end = match &body {
                    SpawnBody::Block(b) => b.span.end,
                    SpawnBody::Expr(e) => e.span().end,
                };
                Ok(Stmt::Spawn {
                    body,
                    span: Span::new(start.start, end),
                })
            }
            TokenKind::If => self.parse_if(),
            TokenKind::While => self.parse_while(),
            TokenKind::For => self.parse_for(),
            TokenKind::Match => self.parse_match(),
            TokenKind::Break => {
                let span = self.bump().span;
                Ok(Stmt::Break { span })
            }
            TokenKind::Continue => {
                let span = self.bump().span;
                Ok(Stmt::Continue { span })
            }
            TokenKind::Return => {
                let start = self.bump().span;
                if matches!(
                    self.peek().kind,
                    TokenKind::RBrace
                        | TokenKind::If
                        | TokenKind::While
                        | TokenKind::For
                        | TokenKind::Match
                        | TokenKind::Return
                        | TokenKind::Break
                        | TokenKind::Continue
                        | TokenKind::Var
                        | TokenKind::Eof
                ) {
                    return Ok(Stmt::Return {
                        value: None,
                        span: start,
                    });
                }
                let value = Some(self.parse_expr()?);
                let end = value.as_ref().unwrap().span().end;
                Ok(Stmt::Return {
                    value,
                    span: Span::new(start.start, end),
                })
            }
            _ => {
                let expr = self.parse_expr()?;
                if matches!(self.peek().kind, TokenKind::Eq) {
                    self.bump();
                    let value = self.parse_expr()?;
                    let end = value.span().end;
                    let target = match expr {
                        Expr::Ident { name, span } => AssignTarget::Local { name, span },
                        Expr::FieldGet {
                            object,
                            field,
                            span,
                        } => AssignTarget::Field {
                            object: *object,
                            field,
                            span,
                        },
                        Expr::Index {
                            array,
                            index,
                            span,
                        } => AssignTarget::Index {
                            array: *array,
                            index: *index,
                            span,
                        },
                        other => {
                            return Err(Diagnostic::new(
                                "invalid assignment target",
                                other.span(),
                            ));
                        }
                    };
                    let start = match &target {
                        AssignTarget::Local { span, .. }
                        | AssignTarget::Field { span, .. }
                        | AssignTarget::Index { span, .. } => span.start,
                    };
                    Ok(Stmt::Assign {
                        target,
                        value,
                        span: Span::new(start, end),
                    })
                } else {
                    let span = expr.span();
                    Ok(Stmt::Expr { expr, span })
                }
            }
        }
    }

    fn parse_if(&mut self) -> Result<Stmt, Diagnostic> {
        let start = self.expect(TokenKind::If)?;
        let mut arms = Vec::new();
        let cond = self.parse_expr()?;
        let body = self.parse_block()?;
        arms.push((cond, body));

        let mut else_block = None;
        while matches!(self.peek().kind, TokenKind::Else) {
            self.bump();
            if matches!(self.peek().kind, TokenKind::If) {
                self.bump();
                let cond = self.parse_expr()?;
                let body = self.parse_block()?;
                arms.push((cond, body));
            } else {
                else_block = Some(self.parse_block()?);
                break;
            }
        }

        let end = else_block
            .as_ref()
            .map(|b| b.span.end)
            .or_else(|| arms.last().map(|(_, b)| b.span.end))
            .unwrap_or(start.end);
        Ok(Stmt::If {
            arms,
            else_block,
            span: Span::new(start.start, end),
        })
    }

    fn parse_while(&mut self) -> Result<Stmt, Diagnostic> {
        let start = self.expect(TokenKind::While)?;
        let cond = self.parse_expr()?;
        let body = self.parse_block()?;
        let end = body.span.end;
        Ok(Stmt::While {
            cond,
            body,
            span: Span::new(start.start, end),
        })
    }

    fn parse_for(&mut self) -> Result<Stmt, Diagnostic> {
        let start = self.expect(TokenKind::For)?;
        let (name, _) = self.expect_ident()?;
        self.expect(TokenKind::In)?;
        let iter = self.parse_expr()?;
        if matches!(self.peek().kind, TokenKind::DotDot) {
            self.bump();
            let range_end = self.parse_expr()?;
            let body = self.parse_block()?;
            let end = body.span.end;
            Ok(Stmt::ForRange {
                name,
                start: iter,
                end: range_end,
                body,
                span: Span::new(start.start, end),
            })
        } else {
            let body = self.parse_block()?;
            let end = body.span.end;
            Ok(Stmt::ForIn {
                name,
                iter,
                body,
                span: Span::new(start.start, end),
            })
        }
    }

    fn parse_match(&mut self) -> Result<Stmt, Diagnostic> {
        let start = self.expect(TokenKind::Match)?;
        let scrutinee = self.parse_expr()?;
        self.expect(TokenKind::LBrace)?;
        let mut arms = Vec::new();
        while !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
            let arm_start = self.peek().span;
            let pattern = self.parse_pattern()?;
            self.expect(TokenKind::FatArrow)?;
            let body = self.parse_block()?;
            let end = body.span.end;
            arms.push(MatchArm {
                pattern,
                body,
                span: Span::new(arm_start.start, end),
            });
        }
        let end = self.expect(TokenKind::RBrace)?;
        Ok(Stmt::Match {
            scrutinee,
            arms,
            span: Span::new(start.start, end.end),
        })
    }

    fn parse_pattern(&mut self) -> Result<Pattern, Diagnostic> {
        let tok = self.peek().clone();
        match tok.kind {
            TokenKind::Underscore => {
                self.bump();
                Ok(Pattern::Wildcard { span: tok.span })
            }
            TokenKind::IntLit(value) => {
                self.bump();
                Ok(Pattern::IntLit {
                    value,
                    span: tok.span,
                })
            }
            TokenKind::Minus => {
                self.bump();
                let lit = self.peek().clone();
                match lit.kind {
                    TokenKind::IntLit(value) => {
                        self.bump();
                        Ok(Pattern::IntLit {
                            value: -value,
                            span: Span::new(tok.span.start, lit.span.end),
                        })
                    }
                    _ => Err(Diagnostic::new(
                        "expected integer literal in match pattern",
                        lit.span,
                    )),
                }
            }
            TokenKind::Ident(ref name) => {
                let name = name.clone();
                self.bump();
                match name.as_str() {
                    "none" => Ok(Pattern::None { span: tok.span }),
                    "ok" | "err" | "some" => {
                        self.expect(TokenKind::LParen)?;
                        let (bind, _) = self.expect_ident()?;
                        let end = self.expect(TokenKind::RParen)?;
                        let span = Span::new(tok.span.start, end.end);
                        Ok(match name.as_str() {
                            "ok" => Pattern::Ok { name: bind, span },
                            "err" => Pattern::Err { name: bind, span },
                            _ => Pattern::Some { name: bind, span },
                        })
                    }
                    _ => Err(Diagnostic::new(
                        "expected match pattern (int, _, ok(x), err(x), some(x), none)",
                        tok.span,
                    )),
                }
            }
            _ => Err(Diagnostic::new(
                "expected match pattern (int, _, ok(x), err(x), some(x), none)",
                tok.span,
            )),
        }
    }

    fn parse_var_decl(&mut self) -> Result<Stmt, Diagnostic> {
        let start = self.expect(TokenKind::Var)?;
        let (name, _) = self.expect_ident()?;
        let ty = if matches!(
            self.peek().kind,
            TokenKind::Int | TokenKind::Float | TokenKind::String | TokenKind::Bool | TokenKind::Ident(_)
        ) && !matches!(self.peek().kind, TokenKind::Eq)
        {
            // Disambiguate: `var x ClassName =` vs `var x =`
            // If next is Ident and following is Eq, it's a type annotation.
            let checkpoint = self.pos;
            if matches!(self.peek().kind, TokenKind::Ident(_)) {
                let _ = self.parse_type()?;
                if matches!(self.peek().kind, TokenKind::Eq) {
                    self.pos = checkpoint;
                    Some(self.parse_type()?)
                } else {
                    self.pos = checkpoint;
                    None
                }
            } else if matches!(
                self.peek().kind,
                TokenKind::Int | TokenKind::Float | TokenKind::String | TokenKind::Bool
            ) {
                Some(self.parse_type()?)
            } else {
                None
            }
        } else {
            None
        };
        self.expect(TokenKind::Eq)?;
        let init = self.parse_expr()?;
        let end = init.span().end;
        Ok(Stmt::VarDecl {
            name,
            ty,
            init,
            span: Span::new(start.start, end),
        })
    }

    fn parse_expr(&mut self) -> Result<Expr, Diagnostic> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_and()?;
        while matches!(self.peek().kind, TokenKind::OrOr) {
            self.bump();
            let right = self.parse_and()?;
            let span = Span::new(left.span().start, right.span().end);
            left = Expr::Binary {
                op: BinOp::Or,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_cmp()?;
        while matches!(self.peek().kind, TokenKind::AndAnd) {
            self.bump();
            let right = self.parse_cmp()?;
            let span = Span::new(left.span().start, right.span().end);
            left = Expr::Binary {
                op: BinOp::And,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_cmp(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_add()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::EqEq => BinOp::Eq,
                TokenKind::Ne => BinOp::Ne,
                TokenKind::Lt => BinOp::Lt,
                TokenKind::Le => BinOp::Le,
                TokenKind::Gt => BinOp::Gt,
                TokenKind::Ge => BinOp::Ge,
                _ => break,
            };
            self.bump();
            let right = self.parse_add()?;
            let span = Span::new(left.span().start, right.span().end);
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_add(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_mul()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::Plus => BinOp::Add,
                TokenKind::Minus => BinOp::Sub,
                _ => break,
            };
            self.bump();
            let right = self.parse_mul()?;
            let span = Span::new(left.span().start, right.span().end);
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_mul(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::Star => BinOp::Mul,
                TokenKind::Slash => BinOp::Div,
                TokenKind::Percent => BinOp::Rem,
                _ => break,
            };
            self.bump();
            let right = self.parse_unary()?;
            let span = Span::new(left.span().start, right.span().end);
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, Diagnostic> {
        if matches!(self.peek().kind, TokenKind::Bang) {
            let start = self.bump().span;
            let expr = self.parse_unary()?;
            let span = Span::new(start.start, expr.span().end);
            return Ok(Expr::Unary {
                op: UnOp::Not,
                expr: Box::new(expr),
                span,
            });
        }
        if matches!(self.peek().kind, TokenKind::Minus) {
            let start = self.bump().span;
            let expr = self.parse_unary()?;
            let span = Span::new(start.start, expr.span().end);
            return Ok(Expr::Unary {
                op: UnOp::Neg,
                expr: Box::new(expr),
                span,
            });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_primary()?;
        loop {
            if matches!(self.peek().kind, TokenKind::Dot) {
                self.bump();
                let (name, name_span) = self.expect_ident()?;
                let start = expr.span().start;
                if matches!(self.peek().kind, TokenKind::LParen) {
                    self.bump();
                    let args = self.parse_arg_list()?;
                    let end = self.expect(TokenKind::RParen)?;
                    expr = Expr::MethodCall {
                        object: Box::new(expr),
                        method: name,
                        args,
                        span: Span::new(start, end.end),
                    };
                } else {
                    expr = Expr::FieldGet {
                        object: Box::new(expr),
                        field: name,
                        span: Span::new(start, name_span.end),
                    };
                }
            } else if matches!(self.peek().kind, TokenKind::LBracket) {
                self.bump();
                let start = expr.span().start;
                let index = self.parse_expr()?;
                let end = self.expect(TokenKind::RBracket)?;
                expr = Expr::Index {
                    array: Box::new(expr),
                    index: Box::new(index),
                    span: Span::new(start, end.end),
                };
            } else if matches!(self.peek().kind, TokenKind::LParen) {
                // Call on a value: `f(args)`, `(fn() int {…})()`, etc.
                // Bare `name(args)` is still parsed as Callee::Func in parse_primary.
                self.bump();
                let start = expr.span().start;
                let args = self.parse_arg_list()?;
                let end = self.expect(TokenKind::RParen)?;
                expr = Expr::Call {
                    callee: Callee::Value {
                        expr: Box::new(expr),
                    },
                    args,
                    span: Span::new(start, end.end),
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, Diagnostic> {
        let tok = self.peek().clone();
        match tok.kind {
            TokenKind::Await => {
                self.bump();
                let expr = self.parse_postfix()?;
                Ok(Expr::Await {
                    span: Span::new(tok.span.start, expr.span().end),
                    expr: Box::new(expr),
                })
            }
            TokenKind::Async => {
                self.bump();
                if !matches!(self.peek().kind, TokenKind::LBrace) {
                    return Err(Diagnostic::new(
                        "expected '{' after async (async blocks are `async { … }`)",
                        self.peek().span,
                    ));
                }
                let body = self.parse_block()?;
                Ok(Expr::AsyncBlock {
                    span: Span::new(tok.span.start, body.span.end),
                    body,
                })
            }
            TokenKind::Fn => self.parse_closure_expr(),
            TokenKind::LBracket => {
                self.bump();
                let mut elems = Vec::new();
                if !matches!(self.peek().kind, TokenKind::RBracket) {
                    loop {
                        elems.push(self.parse_expr()?);
                        if matches!(self.peek().kind, TokenKind::Comma) {
                            self.bump();
                            continue;
                        }
                        break;
                    }
                }
                let end = self.expect(TokenKind::RBracket)?;
                Ok(Expr::ArrayLit {
                    elems,
                    span: Span::new(tok.span.start, end.end),
                })
            }
            TokenKind::IntLit(value) => {
                self.bump();
                Ok(Expr::IntLit {
                    value,
                    span: tok.span,
                })
            }
            TokenKind::FloatLit(value) => {
                self.bump();
                Ok(Expr::FloatLit {
                    value,
                    span: tok.span,
                })
            }
            TokenKind::StringLit(value) => {
                self.bump();
                Ok(Expr::StringLit {
                    value,
                    span: tok.span,
                })
            }
            TokenKind::True => {
                self.bump();
                Ok(Expr::BoolLit {
                    value: true,
                    span: tok.span,
                })
            }
            TokenKind::False => {
                self.bump();
                Ok(Expr::BoolLit {
                    value: false,
                    span: tok.span,
                })
            }
            TokenKind::SelfKw => {
                self.bump();
                Ok(Expr::SelfExpr { span: tok.span })
            }
            TokenKind::Super => {
                self.bump();
                self.expect(TokenKind::Dot)?;
                let (first, first_span) = self.expect_ident()?;
                if matches!(self.peek().kind, TokenKind::Dot) {
                    // super.Base.member
                    self.bump();
                    let (member, member_span) = self.expect_ident()?;
                    if matches!(self.peek().kind, TokenKind::LParen) {
                        self.bump();
                        let args = self.parse_arg_list()?;
                        let end = self.expect(TokenKind::RParen)?;
                        Ok(Expr::SuperMethod {
                            base: Some(first),
                            method: member,
                            args,
                            span: Span::new(tok.span.start, end.end),
                        })
                    } else {
                        Ok(Expr::SuperField {
                            base: Some(first),
                            field: member,
                            span: Span::new(tok.span.start, member_span.end),
                        })
                    }
                } else if matches!(self.peek().kind, TokenKind::LParen) {
                    self.bump();
                    let args = self.parse_arg_list()?;
                    let end = self.expect(TokenKind::RParen)?;
                    Ok(Expr::SuperMethod {
                        base: None,
                        method: first,
                        args,
                        span: Span::new(tok.span.start, end.end),
                    })
                } else {
                    Ok(Expr::SuperField {
                        base: None,
                        field: first,
                        span: Span::new(tok.span.start, first_span.end),
                    })
                }
            }
            TokenKind::New => {
                self.bump();
                let (class_name, _) = self.expect_ident()?;
                self.expect(TokenKind::LParen)?;
                let args = self.parse_arg_list()?;
                let end = self.expect(TokenKind::RParen)?;
                Ok(Expr::New {
                    class_name,
                    args,
                    span: Span::new(tok.span.start, end.end),
                })
            }
            TokenKind::LParen => {
                let start = self.bump().span;
                let expr = self.parse_expr()?;
                let end = self.expect(TokenKind::RParen)?;
                Ok(Expr::Group {
                    expr: Box::new(expr),
                    span: Span::new(start.start, end.end),
                })
            }
            TokenKind::Ident(name) => {
                self.bump();
                if name == "std" && matches!(self.peek().kind, TokenKind::Dot) {
                    self.bump();
                    let (member, member_span) = self.expect_std_member()?;
                    match member.as_str() {
                        "log" | "sleep" | "panic" => {
                            self.expect(TokenKind::LParen)?;
                            let args = self.parse_arg_list()?;
                            let end = self.expect(TokenKind::RParen)?;
                            let callee = match member.as_str() {
                                "log" => Callee::StdLog {
                                    span: Span::new(tok.span.start, member_span.end),
                                },
                                "sleep" => Callee::StdSleep {
                                    span: Span::new(tok.span.start, member_span.end),
                                },
                                _ => Callee::StdPanic {
                                    span: Span::new(tok.span.start, member_span.end),
                                },
                            };
                            return Ok(Expr::Call {
                                callee,
                                args,
                                span: Span::new(tok.span.start, end.end),
                            });
                        }
                        "env" => {
                            self.expect(TokenKind::Dot)?;
                            let (method, method_span) = self.expect_ident()?;
                            self.expect(TokenKind::LParen)?;
                            let args = self.parse_arg_list()?;
                            let end = self.expect(TokenKind::RParen)?;
                            let callee = match method.as_str() {
                                "args" => Callee::StdEnvArgs {
                                    span: Span::new(tok.span.start, method_span.end),
                                },
                                "get" => Callee::StdEnvGet {
                                    span: Span::new(tok.span.start, method_span.end),
                                },
                                "set" => Callee::StdEnvSet {
                                    span: Span::new(tok.span.start, method_span.end),
                                },
                                _ => {
                                    return Err(Diagnostic::new(
                                        "expected std.env.args/get/set",
                                        method_span,
                                    ));
                                }
                            };
                            return Ok(Expr::Call {
                                callee,
                                args,
                                span: Span::new(tok.span.start, end.end),
                            });
                        }
                        "process" => {
                            self.expect(TokenKind::Dot)?;
                            let (method, method_span) = self.expect_ident()?;
                            if method != "exit" {
                                return Err(Diagnostic::new(
                                    "expected std.process.exit(...)",
                                    method_span,
                                ));
                            }
                            self.expect(TokenKind::LParen)?;
                            let args = self.parse_arg_list()?;
                            let end = self.expect(TokenKind::RParen)?;
                            return Ok(Expr::Call {
                                callee: Callee::StdProcessExit {
                                    span: Span::new(tok.span.start, method_span.end),
                                },
                                args,
                                span: Span::new(tok.span.start, end.end),
                            });
                        }
                        "fs" => {
                            self.expect(TokenKind::Dot)?;
                            let (method, method_span) = self.expect_ident()?;
                            self.expect(TokenKind::LParen)?;
                            let args = self.parse_arg_list()?;
                            let end = self.expect(TokenKind::RParen)?;
                            let callee = match method.as_str() {
                                "readToString" => Callee::StdFsReadToString {
                                    span: Span::new(tok.span.start, method_span.end),
                                },
                                "writeString" => Callee::StdFsWriteString {
                                    span: Span::new(tok.span.start, method_span.end),
                                },
                                _ => {
                                    return Err(Diagnostic::new(
                                        "expected std.fs.readToString/writeString",
                                        method_span,
                                    ));
                                }
                            };
                            return Ok(Expr::Call {
                                callee,
                                args,
                                span: Span::new(tok.span.start, end.end),
                            });
                        }
                        "time" => {
                            self.expect(TokenKind::Dot)?;
                            let (method, method_span) = self.expect_ident()?;
                            self.expect(TokenKind::LParen)?;
                            let args = self.parse_arg_list()?;
                            let end = self.expect(TokenKind::RParen)?;
                            let callee = match method.as_str() {
                                "sleepMs" => Callee::StdSleep {
                                    span: Span::new(tok.span.start, method_span.end),
                                },
                                "nowMs" => Callee::StdTimeNowMs {
                                    span: Span::new(tok.span.start, method_span.end),
                                },
                                _ => {
                                    return Err(Diagnostic::new(
                                        "expected std.time.sleepMs/nowMs",
                                        method_span,
                                    ));
                                }
                            };
                            return Ok(Expr::Call {
                                callee,
                                args,
                                span: Span::new(tok.span.start, end.end),
                            });
                        }
                        "string" => {
                            self.expect(TokenKind::Dot)?;
                            let (method, method_span) = self.expect_ident()?;
                            self.expect(TokenKind::LParen)?;
                            let args = self.parse_arg_list()?;
                            let end = self.expect(TokenKind::RParen)?;
                            let callee = match method.as_str() {
                                "len" => Callee::StdStringLen {
                                    span: Span::new(tok.span.start, method_span.end),
                                },
                                "concat" => Callee::StdStringConcat {
                                    span: Span::new(tok.span.start, method_span.end),
                                },
                                "slice" => Callee::StdStringSlice {
                                    span: Span::new(tok.span.start, method_span.end),
                                },
                                "contains" => Callee::StdStringContains {
                                    span: Span::new(tok.span.start, method_span.end),
                                },
                                "fromInt" => Callee::StdStringFromInt {
                                    span: Span::new(tok.span.start, method_span.end),
                                },
                                "parseInt" => Callee::StdStringParseInt {
                                    span: Span::new(tok.span.start, method_span.end),
                                },
                                _ => {
                                    return Err(Diagnostic::new(
                                        "unknown std.string member",
                                        method_span,
                                    ));
                                }
                            };
                            return Ok(Expr::Call {
                                callee,
                                args,
                                span: Span::new(tok.span.start, end.end),
                            });
                        }
                        "List" => {
                            self.expect(TokenKind::Lt)?;
                            let elem_span = self.peek().span;
                            let elem = self.parse_type()?;
                            if matches!(elem, TypeName::Void) {
                                return Err(Diagnostic::new(
                                    "List element type cannot be void",
                                    elem_span,
                                ));
                            }
                            self.expect(TokenKind::Gt)?;
                            self.expect(TokenKind::Dot)?;
                            let (ctor, ctor_span) = self.expect_method_name()?;
                            if ctor != "new" {
                                return Err(Diagnostic::new(
                                    "expected List.new()",
                                    ctor_span,
                                ));
                            }
                            self.expect(TokenKind::LParen)?;
                            let end = self.expect(TokenKind::RParen)?;
                            return Ok(Expr::Call {
                                callee: Callee::StdListNew {
                                    elem: Box::new(elem),
                                    span: Span::new(tok.span.start, end.end),
                                },
                                args: vec![],
                                span: Span::new(tok.span.start, end.end),
                            });
                        }
                        "cpu" => {
                            self.expect(TokenKind::Dot)?;
                            let (method, method_span) = self.expect_ident()?;
                            if method != "submit" {
                                return Err(Diagnostic::new(
                                    "expected std.cpu.submit(...)",
                                    method_span,
                                ));
                            }
                            self.expect(TokenKind::LParen)?;
                            let args = self.parse_arg_list()?;
                            let end = self.expect(TokenKind::RParen)?;
                            return Ok(Expr::Call {
                                callee: Callee::StdCpuSubmit {
                                    span: Span::new(tok.span.start, method_span.end),
                                },
                                args,
                                span: Span::new(tok.span.start, end.end),
                            });
                        }
                        "parallel" => {
                            self.expect(TokenKind::Dot)?;
                            let (method, method_span) = self.expect_ident()?;
                            if method != "map" {
                                return Err(Diagnostic::new(
                                    "expected std.parallel.map(...)",
                                    method_span,
                                ));
                            }
                            self.expect(TokenKind::LParen)?;
                            let args = self.parse_arg_list()?;
                            let end = self.expect(TokenKind::RParen)?;
                            return Ok(Expr::Call {
                                callee: Callee::StdParallelMap {
                                    span: Span::new(tok.span.start, method_span.end),
                                },
                                args,
                                span: Span::new(tok.span.start, end.end),
                            });
                        }
                        "http" => {
                            self.expect(TokenKind::Dot)?;
                            let (method, method_span) = self.expect_ident()?;
                            if method != "get" {
                                return Err(Diagnostic::new(
                                    "expected std.http.get(...)",
                                    method_span,
                                ));
                            }
                            self.expect(TokenKind::LParen)?;
                            let args = self.parse_arg_list()?;
                            let end = self.expect(TokenKind::RParen)?;
                            return Ok(Expr::Call {
                                callee: Callee::StdHttpGet {
                                    span: Span::new(tok.span.start, method_span.end),
                                },
                                args,
                                span: Span::new(tok.span.start, end.end),
                            });
                        }
                        "task" => {
                            self.expect(TokenKind::Dot)?;
                            let (method, method_span) = self.expect_ident()?;
                            match method.as_str() {
                                "yield" => {
                                    self.expect(TokenKind::LParen)?;
                                    let end = self.expect(TokenKind::RParen)?;
                                    return Ok(Expr::Call {
                                        callee: Callee::StdTaskYield {
                                            span: Span::new(tok.span.start, method_span.end),
                                        },
                                        args: vec![],
                                        span: Span::new(tok.span.start, end.end),
                                    });
                                }
                                "CancellationToken" => {
                                    self.expect(TokenKind::Dot)?;
                                    let (ctor, ctor_span) = self.expect_method_name()?;
                                    if ctor != "new" {
                                        return Err(Diagnostic::new(
                                            "expected CancellationToken.new()",
                                            ctor_span,
                                        ));
                                    }
                                    self.expect(TokenKind::LParen)?;
                                    let end = self.expect(TokenKind::RParen)?;
                                    return Ok(Expr::Call {
                                        callee: Callee::StdCancelTokenNew {
                                            span: Span::new(tok.span.start, ctor_span.end),
                                        },
                                        args: vec![],
                                        span: Span::new(tok.span.start, end.end),
                                    });
                                }
                                _ => {
                                    return Err(Diagnostic::new(
                                        "expected std.task.yield or CancellationToken",
                                        method_span,
                                    ));
                                }
                            }
                        }
                        "Result" => {
                            self.expect(TokenKind::Lt)?;
                            let ok_span = self.peek().span;
                            let ok = self.parse_type()?;
                            self.expect(TokenKind::Comma)?;
                            let err_span = self.peek().span;
                            let err = self.parse_type()?;
                            self.expect(TokenKind::Gt)?;
                            if matches!(ok, TypeName::Void) {
                                return Err(Diagnostic::new(
                                    "Result Ok type cannot be void",
                                    ok_span,
                                ));
                            }
                            if matches!(err, TypeName::Void) {
                                return Err(Diagnostic::new(
                                    "Result Err type cannot be void",
                                    err_span,
                                ));
                            }
                            self.expect(TokenKind::Dot)?;
                            let (ctor, ctor_span) = self.expect_method_name()?;
                            self.expect(TokenKind::LParen)?;
                            let args = self.parse_arg_list()?;
                            let end = self.expect(TokenKind::RParen)?;
                            let callee = match ctor.as_str() {
                                "ok" => Callee::StdResultOk {
                                    ok: Box::new(ok),
                                    err: Box::new(err),
                                    span: Span::new(tok.span.start, ctor_span.end),
                                },
                                "err" => Callee::StdResultErr {
                                    ok: Box::new(ok),
                                    err: Box::new(err),
                                    span: Span::new(tok.span.start, ctor_span.end),
                                },
                                _ => {
                                    return Err(Diagnostic::new(
                                        "expected Result.ok(...) or Result.err(...)",
                                        ctor_span,
                                    ));
                                }
                            };
                            return Ok(Expr::Call {
                                callee,
                                args,
                                span: Span::new(tok.span.start, end.end),
                            });
                        }
                        "Option" => {
                            self.expect(TokenKind::Lt)?;
                            let inner_span = self.peek().span;
                            let inner = self.parse_type()?;
                            self.expect(TokenKind::Gt)?;
                            if matches!(inner, TypeName::Void) {
                                return Err(Diagnostic::new(
                                    "Option type cannot be void",
                                    inner_span,
                                ));
                            }
                            self.expect(TokenKind::Dot)?;
                            let (ctor, ctor_span) = self.expect_method_name()?;
                            self.expect(TokenKind::LParen)?;
                            let args = self.parse_arg_list()?;
                            let end = self.expect(TokenKind::RParen)?;
                            let callee = match ctor.as_str() {
                                "some" => Callee::StdOptionSome {
                                    inner: Box::new(inner),
                                    span: Span::new(tok.span.start, ctor_span.end),
                                },
                                "none" => Callee::StdOptionNone {
                                    inner: Box::new(inner),
                                    span: Span::new(tok.span.start, ctor_span.end),
                                },
                                _ => {
                                    return Err(Diagnostic::new(
                                        "expected Option.some(...) or Option.none()",
                                        ctor_span,
                                    ));
                                }
                            };
                            return Ok(Expr::Call {
                                callee,
                                args,
                                span: Span::new(tok.span.start, end.end),
                            });
                        }
                        "sync" => {
                            self.expect(TokenKind::Dot)?;
                            let (kind, kind_span) = self.expect_ident()?;
                            match kind.as_str() {
                                "Channel" => {
                                    self.expect(TokenKind::Lt)?;
                                    let elem_span = self.peek().span;
                                    let elem = self.parse_type()?;
                                    if matches!(elem, TypeName::Void) {
                                        return Err(Diagnostic::new(
                                            "Channel element type cannot be void",
                                            elem_span,
                                        ));
                                    }
                                    self.expect(TokenKind::Gt)?;
                                    self.expect(TokenKind::Dot)?;
                                    let (ctor, ctor_span) = self.expect_method_name()?;
                                    self.expect(TokenKind::LParen)?;
                                    match ctor.as_str() {
                                        "new" => {
                                            let end = self.expect(TokenKind::RParen)?;
                                            return Ok(Expr::Call {
                                                callee: Callee::StdChannelNew {
                                                    elem: Box::new(elem),
                                                    span: Span::new(tok.span.start, end.end),
                                                },
                                                args: vec![],
                                                span: Span::new(tok.span.start, end.end),
                                            });
                                        }
                                        "buffered" => {
                                            let args = self.parse_arg_list()?;
                                            let end = self.expect(TokenKind::RParen)?;
                                            return Ok(Expr::Call {
                                                callee: Callee::StdChannelBuffered {
                                                    elem: Box::new(elem),
                                                    span: Span::new(tok.span.start, ctor_span.end),
                                                },
                                                args,
                                                span: Span::new(tok.span.start, end.end),
                                            });
                                        }
                                        _ => {
                                            return Err(Diagnostic::new(
                                                "expected Channel.new() or Channel.buffered(n)",
                                                ctor_span,
                                            ));
                                        }
                                    }
                                }
                                "WaitGroup" => {
                                    self.expect(TokenKind::Dot)?;
                                    let (ctor, ctor_span) = self.expect_method_name()?;
                                    if ctor != "new" {
                                        return Err(Diagnostic::new(
                                            "expected WaitGroup.new()",
                                            ctor_span,
                                        ));
                                    }
                                    self.expect(TokenKind::LParen)?;
                                    let end = self.expect(TokenKind::RParen)?;
                                    return Ok(Expr::Call {
                                        callee: Callee::StdWaitGroupNew {
                                            span: Span::new(tok.span.start, end.end),
                                        },
                                        args: vec![],
                                        span: Span::new(tok.span.start, end.end),
                                    });
                                }
                                "Mutex" => {
                                    self.expect(TokenKind::Lt)?;
                                    let elem_span = self.peek().span;
                                    let elem = self.parse_type()?;
                                    if matches!(elem, TypeName::Void) {
                                        return Err(Diagnostic::new(
                                            "Mutex element type cannot be void",
                                            elem_span,
                                        ));
                                    }
                                    self.expect(TokenKind::Gt)?;
                                    self.expect(TokenKind::Dot)?;
                                    let (ctor, ctor_span) = self.expect_method_name()?;
                                    if ctor != "new" {
                                        return Err(Diagnostic::new(
                                            "expected Mutex.new(initial)",
                                            ctor_span,
                                        ));
                                    }
                                    self.expect(TokenKind::LParen)?;
                                    let args = self.parse_arg_list()?;
                                    let end = self.expect(TokenKind::RParen)?;
                                    return Ok(Expr::Call {
                                        callee: Callee::StdMutexNew {
                                            elem: Box::new(elem),
                                            span: Span::new(tok.span.start, ctor_span.end),
                                        },
                                        args,
                                        span: Span::new(tok.span.start, end.end),
                                    });
                                }
                                "RwLock" => {
                                    self.expect(TokenKind::Lt)?;
                                    let elem_span = self.peek().span;
                                    let elem = self.parse_type()?;
                                    if matches!(elem, TypeName::Void) {
                                        return Err(Diagnostic::new(
                                            "RwLock element type cannot be void",
                                            elem_span,
                                        ));
                                    }
                                    self.expect(TokenKind::Gt)?;
                                    self.expect(TokenKind::Dot)?;
                                    let (ctor, ctor_span) = self.expect_method_name()?;
                                    if ctor != "new" {
                                        return Err(Diagnostic::new(
                                            "expected RwLock.new(initial)",
                                            ctor_span,
                                        ));
                                    }
                                    self.expect(TokenKind::LParen)?;
                                    let args = self.parse_arg_list()?;
                                    let end = self.expect(TokenKind::RParen)?;
                                    return Ok(Expr::Call {
                                        callee: Callee::StdRwLockNew {
                                            elem: Box::new(elem),
                                            span: Span::new(tok.span.start, ctor_span.end),
                                        },
                                        args,
                                        span: Span::new(tok.span.start, end.end),
                                    });
                                }
                                _ => {
                                    return Err(Diagnostic::new(
                                        format!("unknown std.sync member '{kind}'"),
                                        kind_span,
                                    ));
                                }
                            }
                        }
                        "json" | "yaml" | "toml" | "toon" => {
                            let format = match member.as_str() {
                                "json" => SerdeFormat::Json,
                                "yaml" => SerdeFormat::Yaml,
                                "toml" => SerdeFormat::Toml,
                                _ => SerdeFormat::Toon,
                            };
                            self.expect(TokenKind::Dot)?;
                            let (method, method_span) = self.expect_ident()?;
                            match method.as_str() {
                                "encode" => {
                                    self.expect(TokenKind::LParen)?;
                                    let args = self.parse_arg_list()?;
                                    let end = self.expect(TokenKind::RParen)?;
                                    return Ok(Expr::Call {
                                        callee: Callee::StdSerdeEncode {
                                            format,
                                            span: Span::new(tok.span.start, method_span.end),
                                        },
                                        args,
                                        span: Span::new(tok.span.start, end.end),
                                    });
                                }
                                "decode" => {
                                    let type_arg = if matches!(self.peek().kind, TokenKind::Lt) {
                                        self.bump();
                                        let t = self.parse_type()?;
                                        self.expect(TokenKind::Gt)?;
                                        Some(Box::new(t))
                                    } else {
                                        None
                                    };
                                    self.expect(TokenKind::LParen)?;
                                    let args = self.parse_arg_list()?;
                                    let end = self.expect(TokenKind::RParen)?;
                                    return Ok(Expr::Call {
                                        callee: Callee::StdSerdeDecode {
                                            format,
                                            type_arg,
                                            span: Span::new(tok.span.start, method_span.end),
                                        },
                                        args,
                                        span: Span::new(tok.span.start, end.end),
                                    });
                                }
                                _ => {
                                    return Err(Diagnostic::new(
                                        format!(
                                            "expected std.{}.encode or std.{}.decode",
                                            member, member
                                        ),
                                        method_span,
                                    ));
                                }
                            }
                        }
                        _ => {
                            return Err(Diagnostic::new(
                                format!("unknown std member '{member}'"),
                                member_span,
                            ));
                        }
                    }
                }
                if name == "Future" && matches!(self.peek().kind, TokenKind::Dot) {
                    self.bump();
                    let (method, method_span) = self.expect_ident()?;
                    self.expect(TokenKind::LParen)?;
                    let args = self.parse_arg_list()?;
                    let end = self.expect(TokenKind::RParen)?;
                    let callee = match method.as_str() {
                        "join" => Callee::FutureJoin {
                            span: Span::new(tok.span.start, method_span.end),
                        },
                        "race" => Callee::FutureRace {
                            span: Span::new(tok.span.start, method_span.end),
                        },
                        "ready" => Callee::FutureReady {
                            span: Span::new(tok.span.start, method_span.end),
                        },
                        _ => {
                            return Err(Diagnostic::new(
                                format!("unknown Future member '{method}'"),
                                method_span,
                            ));
                        }
                    };
                    return Ok(Expr::Call {
                        callee,
                        args,
                        span: Span::new(tok.span.start, end.end),
                    });
                }
                // Call: `f(…)` or `f<T>(…)` (not comparison `a < b`).
                if matches!(self.peek().kind, TokenKind::LParen) {
                    self.bump();
                    let args = self.parse_arg_list()?;
                    let end = self.expect(TokenKind::RParen)?;
                    return Ok(Expr::Call {
                        callee: Callee::Func {
                            name,
                            type_args: vec![],
                            span: tok.span,
                        },
                        args,
                        span: Span::new(tok.span.start, end.end),
                    });
                }
                if matches!(self.peek().kind, TokenKind::Lt) {
                    if let Some((type_args, args, end)) = self.try_parse_generic_call()? {
                        return Ok(Expr::Call {
                            callee: Callee::Func {
                                name,
                                type_args,
                                span: tok.span,
                            },
                            args,
                            span: Span::new(tok.span.start, end),
                        });
                    }
                }
                Ok(Expr::Ident {
                    name,
                    span: tok.span,
                })
            }
            _ => Err(Diagnostic::new(
                format!("expected expression, found {:?}", tok.kind),
                tok.span,
            )),
        }
    }

    /// `<T, U>(args)` — returns None if `<` is a comparison, restoring position.
    fn try_parse_generic_call(
        &mut self,
    ) -> Result<Option<(Vec<TypeName>, Vec<Expr>, usize)>, Diagnostic> {
        let save = self.pos;
        if !matches!(self.peek().kind, TokenKind::Lt) {
            return Ok(None);
        }
        match self.parse_call_type_args() {
            Ok(type_args) if matches!(self.peek().kind, TokenKind::LParen) => {
                self.bump();
                let args = self.parse_arg_list()?;
                let end = self.expect(TokenKind::RParen)?;
                Ok(Some((type_args, args, end.end)))
            }
            _ => {
                self.pos = save;
                Ok(None)
            }
        }
    }

    /// `<T, U>` after a call name (not type params of a declaration).
    fn parse_call_type_args(&mut self) -> Result<Vec<TypeName>, Diagnostic> {
        self.expect(TokenKind::Lt)?;
        let mut args = Vec::new();
        loop {
            args.push(self.parse_type()?);
            if matches!(self.peek().kind, TokenKind::Comma) {
                self.bump();
                continue;
            }
            break;
        }
        self.expect(TokenKind::Gt)?;
        Ok(args)
    }

    fn parse_arg_list(&mut self) -> Result<Vec<Expr>, Diagnostic> {
        let mut args = Vec::new();
        if matches!(self.peek().kind, TokenKind::RParen) {
            return Ok(args);
        }
        loop {
            args.push(self.parse_expr()?);
            if matches!(self.peek().kind, TokenKind::Comma) {
                self.bump();
                continue;
            }
            break;
        }
        Ok(args)
    }
}

fn matches_kind(got: &TokenKind, expected: &TokenKind) -> bool {
    match (got, expected) {
        (TokenKind::Ident(_), TokenKind::Ident(_)) => true,
        (TokenKind::IntLit(_), TokenKind::IntLit(_)) => true,
        (TokenKind::FloatLit(_), TokenKind::FloatLit(_)) => true,
        (TokenKind::StringLit(_), TokenKind::StringLit(_)) => true,
        (a, b) => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_control() {
        let src = r#"
@import "std"
fn main() {
    if true {
        std.log("a")
    } else {
        std.log("b")
    }
    while false {
        break
    }
    for i in 0..3 {
        continue
    }
    match 1 {
        0 => { std.log("zero") }
        _ => { std.log("other") }
    }
}
"#;
        let prog = parse(src).unwrap();
        assert_eq!(prog.functions[0].body.stmts.len(), 4);
    }

    #[test]
    fn parses_class() {
        let src = r#"
@import "std"
iclass Named { getName() }
pub class Counter : Named {
    priv var value int
    pub fn new() Counter {
        self.value = 0
        return self
    }
    pub fn get() int { return self.value }
    pub fn getName() { std.log("c") }
}
fn main() {
    var c = new Counter()
    std.log("$1", c.get())
}
"#;
        let prog = parse(src).unwrap();
        assert_eq!(prog.iclasses.len(), 1);
        assert_eq!(prog.classes.len(), 1);
        assert_eq!(prog.classes[0].fields.len(), 1);
        assert_eq!(prog.classes[0].methods.len(), 3);
    }
}
