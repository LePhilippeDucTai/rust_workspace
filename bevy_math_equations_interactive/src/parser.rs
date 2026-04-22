use crate::ast::*;

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Num(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    LParen,
    RParen,
    Comma,
    Equals,
    Eof,
}

struct Lexer<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        Self { chars: input.chars().peekable() }
    }

    fn skip_ws(&mut self) {
        while self.chars.peek().map_or(false, |c| c.is_ascii_whitespace()) {
            self.chars.next();
        }
    }

    fn next_token(&mut self) -> Result<Token, String> {
        self.skip_ws();
        match self.chars.peek().copied() {
            None => Ok(Token::Eof),
            Some('+') => { self.chars.next(); Ok(Token::Plus) }
            Some('-') => { self.chars.next(); Ok(Token::Minus) }
            Some('*') => { self.chars.next(); Ok(Token::Star) }
            Some('/') => { self.chars.next(); Ok(Token::Slash) }
            Some('^') => { self.chars.next(); Ok(Token::Caret) }
            Some('(') => { self.chars.next(); Ok(Token::LParen) }
            Some(')') => { self.chars.next(); Ok(Token::RParen) }
            Some(',') => { self.chars.next(); Ok(Token::Comma) }
            Some('=') => { self.chars.next(); Ok(Token::Equals) }
            Some(c) if c.is_ascii_digit() || c == '.' => {
                let mut s = String::new();
                while self.chars.peek().map_or(false, |c| c.is_ascii_digit() || *c == '.') {
                    s.push(self.chars.next().unwrap());
                }
                s.parse::<f64>().map(Token::Num).map_err(|e| e.to_string())
            }
            Some(c) if c.is_alphabetic() || c == '_' => {
                let mut s = String::new();
                while self.chars.peek().map_or(false, |c| c.is_alphanumeric() || *c == '_') {
                    s.push(self.chars.next().unwrap());
                }
                Ok(Token::Ident(s))
            }
            Some(c) => Err(format!("Unexpected character: '{}'", c)),
        }
    }

    fn tokenize(&mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token()?;
            let done = tok == Token::Eof;
            tokens.push(tok);
            if done { break; }
        }
        Ok(tokens)
    }
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
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn consume(&mut self) -> &Token {
        let tok = self.tokens.get(self.pos).unwrap_or(&Token::Eof);
        self.pos += 1;
        tok
    }

    fn expect(&mut self, expected: &Token) -> Result<(), String> {
        let tok = self.consume().clone();
        if &tok == expected {
            Ok(())
        } else {
            Err(format!("Expected {:?}, got {:?}", expected, tok))
        }
    }

    fn parse_equation(&mut self) -> Result<Equation, String> {
        let lhs = self.parse_expr()?;
        self.expect(&Token::Equals)?;
        let rhs = self.parse_expr()?;
        if self.peek() != &Token::Eof {
            return Err("Unexpected tokens after equation".to_string());
        }
        Ok(Equation { lhs, rhs })
    }

    /// expr := term (('+' | '-') term)*
    fn parse_expr(&mut self) -> Result<Expr, String> {
        let first = self.parse_term()?;
        let mut terms = vec![first];

        loop {
            match self.peek() {
                Token::Plus => {
                    self.consume();
                    terms.push(self.parse_term()?);
                }
                Token::Minus => {
                    self.consume();
                    let t = self.parse_term()?;
                    terms.push(Expr::Neg(Box::new(t)));
                }
                _ => break,
            }
        }

        if terms.len() == 1 {
            Ok(terms.remove(0))
        } else {
            Ok(Expr::Add(terms))
        }
    }

    /// term := power ('*' power | '/' power)*
    fn parse_term(&mut self) -> Result<Expr, String> {
        let first = self.parse_power()?;
        let mut num_factors = vec![first];
        let mut den_factors: Vec<Expr> = vec![];

        loop {
            match self.peek() {
                Token::Star => {
                    self.consume();
                    num_factors.push(self.parse_power()?);
                }
                Token::Slash => {
                    self.consume();
                    den_factors.push(self.parse_power()?);
                }
                _ => break,
            }
        }

        let num = if num_factors.len() == 1 {
            num_factors.remove(0)
        } else {
            Expr::Mul(num_factors)
        };

        if den_factors.is_empty() {
            Ok(num)
        } else {
            let den = if den_factors.len() == 1 {
                den_factors.remove(0)
            } else {
                Expr::Mul(den_factors)
            };
            Ok(Expr::Div(Box::new(num), Box::new(den)))
        }
    }

    /// power := unary ('^' unary)?   (right-associative via recursion)
    fn parse_power(&mut self) -> Result<Expr, String> {
        let base = self.parse_unary()?;
        if self.peek() == &Token::Caret {
            self.consume();
            let exp = self.parse_unary()?;
            Ok(Expr::Pow(Box::new(base), Box::new(exp)))
        } else {
            Ok(base)
        }
    }

    /// unary := '-' unary | atom
    fn parse_unary(&mut self) -> Result<Expr, String> {
        if self.peek() == &Token::Minus {
            self.consume();
            let inner = self.parse_unary()?;
            Ok(Expr::Neg(Box::new(inner)))
        } else {
            self.parse_atom()
        }
    }

    /// atom := num | ident_or_func | '(' expr ')'
    fn parse_atom(&mut self) -> Result<Expr, String> {
        match self.peek().clone() {
            Token::Num(n) => {
                self.consume();
                Ok(Expr::Num(n))
            }
            Token::Ident(name) => {
                self.consume();
                match name.as_str() {
                    "sqrt" => {
                        self.expect(&Token::LParen)?;
                        let arg = self.parse_expr()?;
                        self.expect(&Token::RParen)?;
                        Ok(Expr::Sqrt(Box::new(arg)))
                    }
                    "root" => {
                        self.expect(&Token::LParen)?;
                        let n = self.parse_expr()?;
                        self.expect(&Token::Comma)?;
                        let x = self.parse_expr()?;
                        self.expect(&Token::RParen)?;
                        Ok(Expr::Root(Box::new(n), Box::new(x)))
                    }
                    _ => Ok(Expr::Var(name)),
                }
            }
            Token::LParen => {
                self.consume();
                let inner = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(inner)
            }
            tok => Err(format!("Unexpected token in expression: {:?}", tok)),
        }
    }
}

pub fn parse(input: &str) -> Result<Equation, String> {
    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens);
    parser.parse_equation()
}
