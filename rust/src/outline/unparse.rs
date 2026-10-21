//! Python expressions rendered the way `ast.unparse` renders them, from a tree-sitter tree.
//!
//! The outline and the semantic diff show signatures and values, and the Python build
//! prints them through `ast.unparse`, which normalises quotes, numbers, spacing and
//! parentheses. Reproducing that keeps the two builds' output identical.

use tree_sitter::Node;

/// Operator precedence as in CPython's `ast._Precedence`, lowest first.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Prec {
    NamedExpr,
    Tuple,
    Yield,
    Test,
    Or,
    And,
    Not,
    Cmp,
    BOr,
    BXor,
    BAnd,
    Shift,
    Arith,
    Term,
    Factor,
    Power,
    Await,
    Atom,
}

impl Prec {
    fn next(self) -> Prec {
        use Prec::*;
        match self {
            NamedExpr => Tuple,
            Tuple => Yield,
            Yield => Test,
            Test => Or,
            Or => And,
            And => Not,
            Not => Cmp,
            Cmp => BOr,
            BOr => BXor,
            BXor => BAnd,
            BAnd => Shift,
            Shift => Arith,
            Arith => Term,
            Term => Factor,
            Factor => Power,
            Power => Await,
            Await | Atom => Atom,
        }
    }
}

pub struct Unparser<'a> {
    src: &'a [u8],
}

impl<'a> Unparser<'a> {
    pub fn new(src: &'a [u8]) -> Self {
        Self { src }
    }

    fn text(&self, node: Node) -> &'a str {
        node.utf8_text(self.src).unwrap_or("")
    }

    fn named_children(node: Node<'_>) -> Vec<Node<'_>> {
        let mut cursor = node.walk();
        node.named_children(&mut cursor)
            .filter(|c| c.kind() != "comment")
            .collect()
    }

    fn children(node: Node<'_>) -> Vec<Node<'_>> {
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .filter(|c| c.kind() != "comment")
            .collect()
    }

    /// `ast.unparse(expr)` for a standalone expression (precedence starts at TEST).
    pub fn expr(&self, node: Node) -> String {
        self.at(node, Prec::Test)
    }

    fn wrap(&self, inner: String, own: Prec, wanted: Prec) -> String {
        if own < wanted {
            format!("({inner})")
        } else {
            inner
        }
    }

    fn operator(&self, node: Node) -> String {
        if let Some(op) = node.child_by_field_name("operator") {
            return self.text(op).to_string();
        }
        if node.kind() == "union_type" {
            return "|".into();
        }
        String::new()
    }

    /// Renders `node` in a context that needs at least `wanted` precedence.
    pub fn at(&self, node: Node, wanted: Prec) -> String {
        match node.kind() {
            "parenthesized_expression" => {
                let inner = Self::named_children(node);
                match inner.as_slice() {
                    [only] => self.at(*only, wanted),
                    _ => "()".into(),
                }
            }
            "type" => Self::named_children(node)
                .first()
                .map_or_else(String::new, |n| self.at(*n, wanted)),
            "identifier" | "keyword_identifier" => self.text(node).to_string(),
            "true" => "True".into(),
            "false" => "False".into(),
            "none" => "None".into(),
            "ellipsis" => "...".into(),
            "integer" => int_repr(self.text(node)),
            "float" => float_repr(self.text(node)),
            "string" | "concatenated_string" => self.string(node),
            "attribute" => {
                let object = node
                    .child_by_field_name("object")
                    .expect("attribute object");
                let attr = node
                    .child_by_field_name("attribute")
                    .expect("attribute name");
                let mut base = self.at(object, Prec::Atom);
                // `1 .real`: an int followed directly by a dot would read as a float.
                if object.kind() == "integer" && !base.contains(['.', 'e', 'x', 'j']) {
                    base.push(' ');
                }
                format!("{base}.{}", self.text(attr))
            }
            "call" => {
                let function = node.child_by_field_name("function").expect("call function");
                let args = node
                    .child_by_field_name("arguments")
                    .expect("call arguments");
                // CPython delimits the call and the generator separately: f((x for x in y)).
                let rendered = match args.kind() {
                    "generator_expression" => self.comprehension(args, "(", ")"),
                    _ => self.arguments(args),
                };
                format!("{}({rendered})", self.at(function, Prec::Atom))
            }
            "subscript" => {
                let value = node.child_by_field_name("value").expect("subscript value");
                let mut cursor = node.walk();
                let slices: Vec<Node> = node
                    .children_by_field_name("subscript", &mut cursor)
                    .collect();
                let inner = if let [only] = slices.as_slice() {
                    self.slice_item(*only)
                } else {
                    slices
                        .iter()
                        .map(|s| self.slice_item(*s))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                format!("{}[{inner}]", self.at(value, Prec::Atom))
            }
            "generic_type" => {
                let parts = Self::named_children(node);
                let (name, params) = (parts[0], parts.get(1));
                let inner = params.map_or_else(String::new, |p| {
                    Self::named_children(*p)
                        .iter()
                        .map(|t| self.at(*t, Prec::Test))
                        .collect::<Vec<_>>()
                        .join(", ")
                });
                format!("{}[{inner}]", self.at(name, Prec::Atom))
            }
            "member_type" => Self::named_children(node)
                .iter()
                .map(|n| self.at(*n, Prec::Atom))
                .collect::<Vec<_>>()
                .join("."),
            "splat_type" => format!("*{}", self.text(node).trim_start_matches('*')),
            "slice" => self.slice(node),
            "list" => format!("[{}]", self.items(node)),
            "set" => format!("{{{}}}", self.items(node)),
            "tuple" | "expression_list" | "pattern_list" => {
                let items = Self::named_children(node);
                let mut inner = items
                    .iter()
                    .map(|i| self.at(*i, Prec::Test))
                    .collect::<Vec<_>>()
                    .join(", ");
                if items.len() == 1 {
                    inner.push(',');
                }
                if items.is_empty() || wanted > Prec::Tuple {
                    format!("({inner})")
                } else {
                    inner
                }
            }
            "dictionary" => {
                let items: Vec<String> = Self::named_children(node)
                    .iter()
                    .map(|item| match item.kind() {
                        "pair" => {
                            let key = item.child_by_field_name("key").expect("pair key");
                            let value = item.child_by_field_name("value").expect("pair value");
                            format!(
                                "{}: {}",
                                self.at(key, Prec::Test),
                                self.at(value, Prec::Test)
                            )
                        }
                        _ => self.at(*item, Prec::Test),
                    })
                    .collect();
                format!("{{{}}}", items.join(", "))
            }
            "list_comprehension" => self.comprehension(node, "[", "]"),
            "set_comprehension" => self.comprehension(node, "{", "}"),
            "generator_expression" => self.comprehension(node, "(", ")"),
            "dictionary_comprehension" => self.comprehension(node, "{", "}"),
            "list_splat" | "list_splat_pattern" => {
                format!(
                    "*{}",
                    Self::named_children(node)
                        .first()
                        .map_or_else(String::new, |n| self.at(*n, Prec::BOr))
                )
            }
            "dictionary_splat" | "dictionary_splat_pattern" => {
                format!(
                    "**{}",
                    Self::named_children(node)
                        .first()
                        .map_or_else(String::new, |n| self.at(*n, Prec::BOr))
                )
            }
            "keyword_argument" => {
                let name = node.child_by_field_name("name").expect("keyword name");
                let value = node.child_by_field_name("value").expect("keyword value");
                format!("{}={}", self.text(name), self.at(value, Prec::Test))
            }
            "union_type" => {
                // tree-sitter nests type unions either way round; Python's BinOp chain is
                // left-associative and prints without parentheses.
                let mut operands = Vec::new();
                let mut stack = vec![node];
                while let Some(n) = stack.pop() {
                    let chained = match n.kind() {
                        "union_type" | "type" => true,
                        "binary_operator" => self.operator(n) == "|",
                        _ => false,
                    };
                    if chained {
                        stack.extend(Self::named_children(n).into_iter().rev());
                    } else {
                        operands.push(self.at(n, Prec::BOr.next()));
                    }
                }
                self.wrap(operands.join(" | "), Prec::BOr, wanted)
            }
            "binary_operator" => {
                let op = self.operator(node);
                let own = binop_prec(&op);
                let parts = Self::named_children(node);
                let (left, right) = match node.kind() {
                    "union_type" => (parts[0], parts[parts.len() - 1]),
                    _ => (
                        node.child_by_field_name("left").expect("left"),
                        node.child_by_field_name("right").expect("right"),
                    ),
                };
                // Power binds right to left; everything else left to right.
                let (lp, rp) = if op == "**" {
                    (own.next(), own)
                } else {
                    (own, own.next())
                };
                let inner = format!("{} {op} {}", self.at(left, lp), self.at(right, rp));
                self.wrap(inner, own, wanted)
            }
            "unary_operator" => {
                let op = self.operator(node);
                let arg = node
                    .child_by_field_name("argument")
                    .expect("unary argument");
                let inner = format!("{op}{}", self.at(arg, Prec::Factor));
                self.wrap(inner, Prec::Factor, wanted)
            }
            "not_operator" => {
                let arg = node.child_by_field_name("argument").expect("not argument");
                let inner = format!("not {}", self.at(arg, Prec::Not));
                self.wrap(inner, Prec::Not, wanted)
            }
            "boolean_operator" => {
                let op = self.operator(node);
                let own = if op == "or" { Prec::Or } else { Prec::And };
                // `a or b or c` is one BoolOp with three values, and CPython raises the
                // required precedence by one level for each value in turn.
                let mut values = Vec::new();
                let mut current = node;
                loop {
                    values.push(current.child_by_field_name("right").expect("right"));
                    let left = current.child_by_field_name("left").expect("left");
                    if left.kind() == "boolean_operator" && self.operator(left) == op {
                        current = left;
                    } else {
                        values.push(left);
                        break;
                    }
                }
                values.reverse();
                let mut level = own;
                let rendered: Vec<String> = values
                    .iter()
                    .map(|v| {
                        level = level.next();
                        self.at(*v, level)
                    })
                    .collect();
                self.wrap(rendered.join(&format!(" {op} ")), own, wanted)
            }
            "comparison_operator" => {
                let parts: Vec<String> = Self::children(node)
                    .iter()
                    .map(|c| {
                        if c.is_named() {
                            self.at(*c, Prec::Cmp.next())
                        } else {
                            self.text(*c).to_string()
                        }
                    })
                    .collect();
                // Multi-word operators (`not in`, `is not`) arrive as separate tokens.
                let mut inner = String::new();
                for part in parts {
                    if !inner.is_empty() {
                        inner.push(' ');
                    }
                    inner.push_str(&part);
                }
                self.wrap(inner, Prec::Cmp, wanted)
            }
            "conditional_expression" => {
                let parts = Self::named_children(node);
                let inner = format!(
                    "{} if {} else {}",
                    self.at(parts[0], Prec::Test.next()),
                    self.at(parts[1], Prec::Test.next()),
                    self.at(parts[2], Prec::Test)
                );
                self.wrap(inner, Prec::Test, wanted)
            }
            "lambda" => {
                let params = node
                    .child_by_field_name("parameters")
                    .map_or_else(String::new, |p| self.parameters(p));
                let body = node.child_by_field_name("body").expect("lambda body");
                let head = if params.is_empty() {
                    "lambda".to_string()
                } else {
                    format!("lambda {params}")
                };
                self.wrap(
                    format!("{head}: {}", self.at(body, Prec::Test)),
                    Prec::Test,
                    wanted,
                )
            }
            "named_expression" => {
                let name = node.child_by_field_name("name").expect("walrus name");
                let value = node.child_by_field_name("value").expect("walrus value");
                let inner = format!("{} := {}", self.text(name), self.at(value, Prec::Atom));
                self.wrap(inner, Prec::NamedExpr, wanted)
            }
            "await" => {
                let arg = Self::named_children(node)[0];
                self.wrap(
                    format!("await {}", self.at(arg, Prec::Atom)),
                    Prec::Await,
                    wanted,
                )
            }
            "yield" => {
                let rest = self.text(node).trim_start_matches("yield").trim();
                let inner = if rest.is_empty() {
                    "yield".into()
                } else {
                    format!("yield {rest}")
                };
                self.wrap(inner, Prec::Yield, wanted.max(Prec::Atom))
            }
            _ => self.text(node).to_string(),
        }
    }

    fn items(&self, node: Node) -> String {
        Self::named_children(node)
            .iter()
            .map(|i| self.at(*i, Prec::Test))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn arguments(&self, args: Node) -> String {
        Self::named_children(args)
            .iter()
            .map(|a| self.at(*a, Prec::Test))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn slice_item(&self, node: Node) -> String {
        match node.kind() {
            // `a[1, 2]` keeps its tuple bare; `a[()]` still needs the parentheses.
            "tuple" if Self::named_children(node).len() > 1 => Self::named_children(node)
                .iter()
                .map(|i| self.at(*i, Prec::Test))
                .collect::<Vec<_>>()
                .join(", "),
            _ => self.at(node, Prec::Test),
        }
    }

    fn slice(&self, node: Node) -> String {
        // Children are expressions separated by ':' tokens; empty parts are omitted.
        let mut parts = vec![String::new()];
        for child in Self::children(node) {
            if !child.is_named() && self.text(child) == ":" {
                parts.push(String::new());
            } else if child.is_named() {
                *parts.last_mut().expect("part") = self.at(child, Prec::Test);
            }
        }
        if parts.len() == 3 && parts[2].is_empty() {
            parts.pop();
        }
        parts.join(":")
    }

    fn comprehension(&self, node: Node, open: &str, close: &str) -> String {
        let mut out = String::new();
        for child in Self::named_children(node) {
            match child.kind() {
                "for_in_clause" => {
                    let is_async = Self::children(child)
                        .first()
                        .is_some_and(|c| self.text(*c) == "async");
                    let left = child.child_by_field_name("left").expect("for target");
                    let mut cursor = child.walk();
                    let rights: Vec<Node> =
                        child.children_by_field_name("right", &mut cursor).collect();
                    let target = self.at(left, Prec::Tuple);
                    let iter = rights
                        .iter()
                        .map(|r| self.at(*r, Prec::Test.next()))
                        .collect::<Vec<_>>()
                        .join(", ");
                    out.push_str(if is_async { " async for " } else { " for " });
                    out.push_str(&format!("{target} in {iter}"));
                }
                "if_clause" => {
                    let cond = Self::named_children(child)[0];
                    out.push_str(&format!(" if {}", self.at(cond, Prec::Test.next())));
                }
                "pair" => {
                    let key = child.child_by_field_name("key").expect("key");
                    let value = child.child_by_field_name("value").expect("value");
                    out.insert_str(
                        0,
                        &format!(
                            "{}: {}",
                            self.at(key, Prec::Test),
                            self.at(value, Prec::Test)
                        ),
                    );
                }
                _ => out.insert_str(0, &self.at(child, Prec::Test)),
            }
        }
        format!("{open}{out}{close}")
    }

    /// `ast.unparse(arguments)`: `a, b=1, /, c: int=2, *args, d, **kw`.
    pub fn parameters(&self, params: Node) -> String {
        let mut parts = Vec::new();
        for p in Self::named_children(params) {
            let part = match p.kind() {
                "identifier" => self.text(p).to_string(),
                "typed_parameter" => {
                    let name = Self::named_children(p)[0];
                    let ty = p.child_by_field_name("type").expect("annotation");
                    format!("{}: {}", self.at(name, Prec::Test), self.at(ty, Prec::Test))
                }
                "default_parameter" => {
                    let name = p.child_by_field_name("name").expect("name");
                    let value = p.child_by_field_name("value").expect("default");
                    format!("{}={}", self.text(name), self.at(value, Prec::Test))
                }
                "typed_default_parameter" => {
                    let name = p.child_by_field_name("name").expect("name");
                    let ty = p.child_by_field_name("type").expect("annotation");
                    let value = p.child_by_field_name("value").expect("default");
                    format!(
                        "{}: {}={}",
                        self.text(name),
                        self.at(ty, Prec::Test),
                        self.at(value, Prec::Test)
                    )
                }
                "list_splat_pattern" | "dictionary_splat_pattern" => self.at(p, Prec::Test),
                "keyword_separator" => "*".into(),
                "positional_separator" => "/".into(),
                _ => self.text(p).to_string(),
            };
            parts.push(part);
        }
        parts.join(", ")
    }

    /// A string literal (or implicit concatenation) as Python's `repr` of its value.
    fn string(&self, node: Node) -> String {
        let pieces: Vec<Node> = if node.kind() == "concatenated_string" {
            Self::named_children(node)
        } else {
            vec![node]
        };
        let prefixes: Vec<String> = pieces
            .iter()
            .map(|p| split_literal(self.text(*p)).0.to_ascii_lowercase())
            .collect();
        if prefixes.iter().any(|p| p.contains('t')) {
            return self.text(node).to_string();
        }
        if prefixes.iter().any(|p| p.contains('f')) {
            return self.fstring(&pieces);
        }
        let mut value = String::new();
        let mut bytes = false;
        for piece in &pieces {
            let raw = self.text(*piece);
            let (prefix, body) = split_literal(raw);
            let lower = prefix.to_ascii_lowercase();
            bytes |= lower.contains('b');
            if lower.contains('r') {
                value.push_str(body);
            } else {
                value.push_str(&decode_escapes(body));
            }
        }
        if bytes {
            bytes_repr(&value)
        } else if prefixes.first().is_some_and(|p| p.contains('u')) {
            // CPython keeps a leading u'' as Constant.kind and writes it back.
            format!("u{}", str_repr(&value))
        } else {
            str_repr(&value)
        }
    }

    /// An f-string (possibly concatenated with plain strings) as CPython 3.14's
    /// `_write_ftstring` writes it: constant parts escaped, expressions re-rendered, and the
    /// first quote style that clashes with neither.
    fn fstring(&self, pieces: &[Node]) -> String {
        let mut parts: Vec<(String, bool)> = Vec::new();
        let push_const = |parts: &mut Vec<(String, bool)>, text: String| match parts.last_mut() {
            Some((last, true)) => last.push_str(&text),
            _ => parts.push((text, true)),
        };
        for piece in pieces {
            let (prefix, _) = split_literal(self.text(*piece));
            let lower = prefix.to_ascii_lowercase();
            let raw = lower.contains('r');
            if !lower.contains('f') {
                let (_, body) = split_literal(self.text(*piece));
                push_const(
                    &mut parts,
                    if raw {
                        body.to_string()
                    } else {
                        decode_escapes(body)
                    },
                );
                continue;
            }
            for child in Self::children(*piece) {
                match child.kind() {
                    "string_content" => {
                        let text = self.text(child).replace("{{", "{").replace("}}", "}");
                        push_const(&mut parts, if raw { text } else { decode_escapes(&text) });
                    }
                    "interpolation" => {
                        let (debug, rendered) = self.interpolation(child);
                        if let Some(debug) = debug {
                            push_const(&mut parts, debug);
                        }
                        parts.push((rendered, false));
                    }
                    _ => {}
                }
            }
        }

        let mut quotes: Vec<&str> = vec!["'", "\"", "\"\"\"", "'''"];
        let mut rendered = Vec::new();
        for (value, constant) in parts {
            if constant {
                let escaped: String = value
                    .replace('{', "{{")
                    .replace('}', "}}")
                    .chars()
                    .map(|c| {
                        if c == '\\' || !is_printable(c) {
                            unicode_escape(c)
                        } else {
                            c.to_string()
                        }
                    })
                    .collect();
                let mut possible: Vec<&str> = quotes
                    .iter()
                    .copied()
                    .filter(|q| !escaped.contains(q))
                    .collect();
                if escaped.contains('\n') {
                    possible.retain(|q| q.len() == 3);
                }
                if possible.is_empty() {
                    return self.text(pieces[0]).to_string();
                }
                if let Some(last) = escaped.chars().last() {
                    possible.sort_by_key(|q| q.starts_with(last));
                }
                quotes = possible;
                rendered.push(escaped);
            } else {
                if value.contains('\n') {
                    quotes.retain(|q| q.len() == 3);
                }
                let narrowed: Vec<&str> = quotes
                    .iter()
                    .copied()
                    .filter(|q| !value.contains(q))
                    .collect();
                if !narrowed.is_empty() {
                    quotes = narrowed;
                }
                rendered.push(value);
            }
        }
        let quote = quotes.first().copied().unwrap_or("'");
        format!("f{quote}{}{quote}", rendered.concat())
    }

    /// `{expr!r:spec}` as `visit_FormattedValue` writes it, plus the constant that a
    /// self-documenting `{expr=}` expands to.
    fn interpolation(&self, node: Node) -> (Option<String>, String) {
        let expr = node
            .child_by_field_name("expression")
            .expect("interpolation expression");
        let mut conversion = None;
        let mut spec = None;
        let mut debug = false;
        for child in Self::children(node) {
            match child.kind() {
                "type_conversion" => conversion = Some(self.text(child).to_string()),
                "format_specifier" => spec = Some(child),
                "=" => debug = true,
                _ => {}
            }
        }
        let debug_text = debug.then(|| {
            let start = node.start_byte() + 1;
            let end = Self::children(node)
                .iter()
                .find(|c| c.kind() == "=")
                .map_or(start, |c| c.end_byte());
            String::from_utf8_lossy(&self.src[start..end]).into_owned()
        });
        if debug && conversion.is_none() && spec.is_none() {
            conversion = Some("!r".into());
        }
        let inner = self.at(expr, Prec::Test.next());
        let mut out = String::from("{");
        if inner.starts_with('{') {
            out.push(' ');
        }
        out.push_str(&inner);
        if let Some(c) = conversion {
            out.push_str(&c);
        }
        if let Some(spec) = spec {
            out.push(':');
            // The literal text of a spec is not a child node: take the source between the
            // nested interpolations.
            let escape = |s: &str| {
                s.replace('\\', "\\\\")
                    .replace('\'', "\\'")
                    .replace('"', "\\\"")
                    .replace('\n', "\\n")
            };
            let mut cursor = spec.start_byte();
            if self.src.get(cursor) == Some(&b':') {
                cursor += 1;
            }
            for part in Self::children(spec)
                .into_iter()
                .filter(|p| p.kind() == "interpolation")
            {
                out.push_str(&escape(&String::from_utf8_lossy(
                    &self.src[cursor..part.start_byte()],
                )));
                out.push_str(&self.interpolation(part).1);
                cursor = part.end_byte();
            }
            out.push_str(&escape(&String::from_utf8_lossy(
                &self.src[cursor..spec.end_byte()],
            )));
        }
        out.push('}');
        (debug_text, out)
    }
}

/// Python's `unicode_escape` codec for one character.
fn unicode_escape(c: char) -> String {
    match c {
        '\\' => "\\\\".into(),
        '\n' => "\\n".into(),
        '\r' => "\\r".into(),
        '\t' => "\\t".into(),
        c if (c as u32) < 0x100 => format!("\\x{:02x}", c as u32),
        c if (c as u32) < 0x10000 => format!("\\u{:04x}", c as u32),
        c => format!("\\U{:08x}", c as u32),
    }
}

fn binop_prec(op: &str) -> Prec {
    match op {
        "|" => Prec::BOr,
        "^" => Prec::BXor,
        "&" => Prec::BAnd,
        "<<" | ">>" => Prec::Shift,
        "+" | "-" => Prec::Arith,
        "*" | "/" | "//" | "%" | "@" => Prec::Term,
        "**" => Prec::Power,
        _ => Prec::Atom,
    }
}

/// Prefix letters and the text between the quotes of one literal.
pub fn split_literal(raw: &str) -> (&str, &str) {
    let quote_at = raw.find(['\'', '"']).unwrap_or(0);
    let (prefix, rest) = raw.split_at(quote_at);
    let quote = if rest.starts_with("\"\"\"") || rest.starts_with("'''") {
        3
    } else {
        1
    };
    if rest.len() < 2 * quote {
        return (prefix, "");
    }
    (prefix, &rest[quote..rest.len() - quote])
}

pub fn decode_escapes(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let Some(next) = chars.next() else {
            out.push('\\');
            break;
        };
        match next {
            '\n' => {}
            '\\' => out.push('\\'),
            '\'' => out.push('\''),
            '"' => out.push('"'),
            'a' => out.push('\x07'),
            'b' => out.push('\x08'),
            'f' => out.push('\x0c'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'v' => out.push('\x0b'),
            '0'..='7' => {
                let mut code = next.to_digit(8).unwrap_or(0);
                for _ in 0..2 {
                    match chars.peek().and_then(|d| d.to_digit(8)) {
                        Some(d) => {
                            code = code * 8 + d;
                            chars.next();
                        }
                        None => break,
                    }
                }
                out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
            }
            'x' | 'u' | 'U' => {
                let width = match next {
                    'x' => 2,
                    'u' => 4,
                    _ => 8,
                };
                let hex: String = (0..width).filter_map(|_| chars.next()).collect();
                match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    Some(ch) if hex.len() == width => out.push(ch),
                    _ => {
                        out.push('\\');
                        out.push(next);
                        out.push_str(&hex);
                    }
                }
            }
            other => {
                out.push('\\');
                out.push(other);
            }
        }
    }
    out
}

/// `repr(str)`: single quotes unless the text holds a single quote and no double quote.
pub fn str_repr(value: &str) -> String {
    let quote = if value.contains('\'') && !value.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::with_capacity(value.len() + 2);
    out.push(quote);
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if is_printable(c) => out.push(c),
            c if (c as u32) < 0x100 => out.push_str(&format!("\\x{:02x}", c as u32)),
            c if (c as u32) < 0x10000 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push_str(&format!("\\U{:08x}", c as u32)),
        }
    }
    out.push(quote);
    out
}

fn bytes_repr(value: &str) -> String {
    let quote = if value.contains('\'') && !value.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = format!("b{quote}");
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if (' '..='~').contains(&c) => out.push(c),
            c => out.push_str(&format!("\\x{:02x}", c as u32 & 0xff)),
        }
    }
    out.push(quote);
    out
}

/// Close to `str.isprintable`: control and separator characters other than the space
/// are escaped.
fn is_printable(c: char) -> bool {
    !(c.is_control()
        || matches!(c, '\u{2028}' | '\u{2029}' | '\u{00a0}' | '\u{00ad}')
        || (c.is_whitespace() && c != ' '))
}

/// `repr(int(...))`: underscores, hex, octal and binary literals become decimal.
pub fn int_repr(raw: &str) -> String {
    let (neg, body) = match raw.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, raw),
    };
    let clean = body.replace('_', "");
    if clean.ends_with(['j', 'J']) {
        return format!("{}{}", if neg { "-" } else { "" }, imag_repr(&clean));
    }
    let lower = clean.to_ascii_lowercase();
    let parsed = if let Some(hex) = lower.strip_prefix("0x") {
        u128::from_str_radix(hex, 16).ok()
    } else if let Some(oct) = lower.strip_prefix("0o") {
        u128::from_str_radix(oct, 8).ok()
    } else if let Some(bin) = lower.strip_prefix("0b") {
        u128::from_str_radix(bin, 2).ok()
    } else {
        lower.parse::<u128>().ok()
    };
    let digits = parsed.map_or(clean, |v| v.to_string());
    if neg { format!("-{digits}") } else { digits }
}

fn imag_repr(clean: &str) -> String {
    let number = &clean[..clean.len() - 1];
    match number.parse::<f64>() {
        Ok(v) if v.fract() == 0.0 && v.abs() < 1e16 => format!("{}j", v as i64),
        Ok(v) => format!("{}j", py_float(v)),
        Err(_) => clean.to_string(),
    }
}

/// `repr(float(...))`, including the switch to exponent form outside 1e-4 .. 1e16.
pub fn float_repr(raw: &str) -> String {
    let clean = raw.replace('_', "");
    if clean.ends_with(['j', 'J']) {
        return imag_repr(&clean);
    }
    match clean.parse::<f64>() {
        Ok(v) => py_float(v),
        Err(_) => clean,
    }
}

fn py_float(v: f64) -> String {
    if v.is_infinite() {
        // ast.unparse writes infinity as an overflowing literal.
        return if v > 0.0 {
            "1e309".into()
        } else {
            "-1e309".into()
        };
    }
    if v == 0.0 {
        return if v.is_sign_negative() {
            "-0.0".into()
        } else {
            "0.0".into()
        };
    }
    // Shortest round-trip digits and the decimal exponent, then Python's layout rules.
    let sci = format!("{v:e}");
    let (mantissa, exp) = sci.split_once('e').expect("exponent");
    let exp: i32 = exp.parse().expect("exponent number");
    let negative = mantissa.starts_with('-');
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let sign = if negative { "-" } else { "" };
    if !(-4..16).contains(&exp) {
        let (first, rest) = digits.split_at(1);
        let frac = if rest.is_empty() {
            String::new()
        } else {
            format!(".{rest}")
        };
        let e_sign = if exp < 0 { "-" } else { "+" };
        return format!("{sign}{first}{frac}e{e_sign}{:02}", exp.abs());
    }
    let point = exp + 1;
    if point <= 0 {
        format!("{sign}0.{}{digits}", "0".repeat((-point) as usize))
    } else if point as usize >= digits.len() {
        format!(
            "{sign}{digits}{}.0",
            "0".repeat(point as usize - digits.len())
        )
    } else {
        let (int, frac) = digits.split_at(point as usize);
        format!("{sign}{int}.{frac}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_print_like_python_repr() {
        assert_eq!(int_repr("1_000"), "1000");
        assert_eq!(int_repr("0x10"), "16");
        assert_eq!(float_repr("1."), "1.0");
        assert_eq!(float_repr(".5"), "0.5");
        assert_eq!(float_repr("1e16"), "1e+16");
        assert_eq!(float_repr("1.5e-5"), "1.5e-05");
        assert_eq!(float_repr("1e3"), "1000.0");
        assert_eq!(float_repr("0.1"), "0.1");
        assert_eq!(float_repr("123456789.125"), "123456789.125");
    }

    #[test]
    fn strings_print_like_python_repr() {
        assert_eq!(str_repr("it's"), "\"it's\"");
        assert_eq!(str_repr("a\"b'c"), "'a\"b\\'c'");
        assert_eq!(str_repr("tab\there"), "'tab\\there'");
        assert_eq!(decode_escapes(r"\x41\n\101"), "A\nA");
    }
}
