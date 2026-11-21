//! The token parser, written with chumsky. Newlines carry no meaning, so it works on tokens only.

// chumsky's `select!` macro expands to a closure returning `Simple<Token>`, which clippy thinks is large.
// The type is chumsky's own, so there is nothing to fix here.
#![allow(clippy::result_large_err)]

use chumsky::prelude::*;

use crate::ast::*;
use crate::token::Token;

pub(crate) type E = Simple<Token>;

fn ident() -> impl Parser<Token, Ident, Error = E> + Clone {
    select! { Token::Ident(name) => name }
        .map_with_span(|name, span| Ident { name, span })
        .labelled("a name")
}

/// A keyword: an identifier with this exact text.
fn kw(word: &'static str) -> impl Parser<Token, (), Error = E> + Clone {
    just(Token::Ident(word.to_string())).ignored()
}

fn path() -> impl Parser<Token, Path, Error = E> + Clone {
    ident()
        .separated_by(just(Token::Dot))
        .at_least(1)
        .map_with_span(|segments, span| Path { segments, span })
}

fn doc() -> impl Parser<Token, Vec<String>, Error = E> + Clone {
    select! { Token::Doc(line) => line }.repeated()
}

fn string() -> impl Parser<Token, (String, crate::token::Span), Error = E> + Clone {
    select! { Token::Str(s) => s }
        .map_with_span(|s, span| (s, span))
        .labelled("a string")
}

fn annotations() -> impl Parser<Token, Vec<Annotation>, Error = E> + Clone {
    let arg = select! {
        Token::Str(s) => AnnotationArg::Str(s),
        Token::Num(n) => AnnotationArg::Num(n),
        Token::Ident(i) => AnnotationArg::Ident(i),
    };
    let args = arg
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .delimited_by(just(Token::LParen), just(Token::RParen));
    just(Token::At)
        .ignore_then(ident())
        .then(args.or_not())
        .map_with_span(|(name, args), span| Annotation {
            name,
            args: args.unwrap_or_default(),
            span,
        })
        .repeated()
}

fn field<T>(ty: T) -> impl Parser<Token, Field, Error = E> + Clone
where
    T: Parser<Token, TypeExpr, Error = E> + Clone,
{
    doc()
        .then(ident())
        .then(just(Token::Question).or_not().map(|q| q.is_some()))
        .then_ignore(just(Token::Colon))
        .then(ty)
        .then(annotations())
        .map_with_span(|((((doc, name), optional), ty), annotations), span| Field {
            doc,
            name,
            optional,
            ty,
            annotations,
            span,
        })
        .then_ignore(just(Token::Comma).or_not())
}

fn braced_fields<T>(ty: T) -> impl Parser<Token, Vec<Field>, Error = E> + Clone
where
    T: Parser<Token, TypeExpr, Error = E> + Clone,
{
    field(ty)
        .repeated()
        .delimited_by(just(Token::LBrace), just(Token::RBrace))
}

fn type_expr() -> impl Parser<Token, TypeExpr, Error = E> + Clone {
    recursive(|ty| {
        let inline = braced_fields(ty.clone())
            .map(TypeKind::Inline)
            .map_with_span(|kind, span| TypeExpr { kind, span });
        let args = ty
            .separated_by(just(Token::Comma))
            .allow_trailing()
            .at_least(1)
            .delimited_by(just(Token::Lt), just(Token::Gt));
        let named = path()
            .then(args.or_not())
            .map_with_span(|(path, args), span| TypeExpr {
                kind: TypeKind::Named {
                    path,
                    args: args.unwrap_or_default(),
                },
                span,
            });
        inline
            .or(named)
            .then(
                just(Token::LBracket)
                    .ignore_then(just(Token::RBracket))
                    .map_with_span(|_, span| span)
                    .repeated(),
            )
            .foldl(|inner, bracket| {
                let span = inner.span.start..bracket.end;
                TypeExpr {
                    kind: TypeKind::List(Box::new(inner)),
                    span,
                }
            })
    })
    .labelled("a type")
}

/// `-> [stream] A | B? | C`
fn result<T>(ty: T) -> impl Parser<Token, ResultType, Error = E> + Clone
where
    T: Parser<Token, TypeExpr, Error = E> + Clone,
{
    let variant = ty
        .then(just(Token::Question).map_with_span(|_, span| span).or_not())
        .map(|(ty, q)| match q {
            Some(q) => TypeExpr {
                span: ty.span.start..q.end,
                kind: TypeKind::Optional(Box::new(ty)),
            },
            None => ty,
        });
    just(Token::Arrow)
        .ignore_then(kw("stream").or_not().map(|s| s.is_some()))
        .then(variant.separated_by(just(Token::Pipe)).at_least(1))
        .map_with_span(|(stream, mut variants), span| ResultType {
            stream,
            ty: if variants.len() == 1 {
                variants.remove(0)
            } else {
                TypeExpr {
                    span: variants[0].span.start..variants[variants.len() - 1].span.end,
                    kind: TypeKind::Union(variants),
                }
            },
            span,
        })
}

/// `(id: uuid, page?: int)` or `(CreateOrder)` or `(stream Item)`
fn params<T>(ty: T) -> impl Parser<Token, Vec<Param>, Error = E> + Clone
where
    T: Parser<Token, TypeExpr, Error = E> + Clone,
{
    let named = ident()
        .then(just(Token::Question).or_not())
        .then_ignore(just(Token::Colon))
        .then(ty.clone())
        .map_with_span(|((name, q), ty), span| Param {
            name: Some(name),
            optional: q.is_some(),
            stream: false,
            ty,
            span,
        });
    let unnamed = kw("stream")
        .or_not()
        .then(ty)
        .map_with_span(|(stream, ty), span| Param {
            name: None,
            optional: false,
            stream: stream.is_some(),
            ty,
            span,
        });
    named
        .or(unnamed)
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .delimited_by(just(Token::LParen), just(Token::RParen))
}

fn enum_decl() -> impl Parser<Token, DeclKind, Error = E> + Clone {
    let variant = doc()
        .then(ident())
        .then(annotations())
        .map_with_span(|((doc, name), annotations), span| Variant {
            doc,
            name,
            annotations,
            span,
        })
        .then_ignore(just(Token::Comma).or_not());
    kw("enum")
        .ignore_then(ident())
        .then(annotations())
        .then(
            variant
                .repeated()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .map(|((name, annotations), variants)| {
            DeclKind::Enum(EnumDecl {
                name,
                annotations,
                variants,
            })
        })
}

fn type_decl<T>(ty: T) -> impl Parser<Token, DeclKind, Error = E> + Clone
where
    T: Parser<Token, TypeExpr, Error = E> + Clone,
{
    let generics = ident()
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .at_least(1)
        .delimited_by(just(Token::Lt), just(Token::Gt));
    kw("type")
        .ignore_then(ident())
        .then(generics.or_not())
        .then(annotations())
        .then(braced_fields(ty))
        .map(|(((name, params), annotations), fields)| {
            DeclKind::Type(TypeDecl {
                name,
                params: params.unwrap_or_default(),
                annotations,
                fields,
            })
        })
}

fn error_decl<T>(ty: T) -> impl Parser<Token, DeclKind, Error = E> + Clone
where
    T: Parser<Token, TypeExpr, Error = E> + Clone,
{
    kw("error")
        .ignore_then(ident())
        .then(annotations())
        .then(braced_fields(ty))
        .map(|((name, annotations), fields)| {
            DeclKind::Error(ErrorDecl {
                name,
                annotations,
                fields,
            })
        })
}

fn event_decl<T>(ty: T) -> impl Parser<Token, DeclKind, Error = E> + Clone
where
    T: Parser<Token, TypeExpr, Error = E> + Clone,
{
    let key = kw("key")
        .ignore_then(just(Token::Colon))
        .ignore_then(path())
        .map_with_span(|path, span| EventOption::Key { path, span });
    let headers = kw("headers")
        .ignore_then(just(Token::Colon))
        .ignore_then(ty.clone())
        .map_with_span(|ty, span| EventOption::Headers { ty, span });
    let options = key
        .or(headers)
        .then_ignore(just(Token::Comma).or_not())
        .repeated()
        .delimited_by(just(Token::LBrace), just(Token::RBrace));
    kw("event")
        .ignore_then(path())
        .then_ignore(just(Token::Colon))
        .then(ty)
        .then(options.or_not())
        .then(annotations())
        .map(|(((name, ty), options), annotations)| {
            DeclKind::Event(EventDecl {
                name,
                ty,
                options: options.unwrap_or_default(),
                annotations,
            })
        })
}

fn http_decl<T>(ty: T) -> impl Parser<Token, DeclKind, Error = E> + Clone
where
    T: Parser<Token, TypeExpr, Error = E> + Clone,
{
    let base = select! { Token::Path(p) => p }
        .map_with_span(|p, span| (p, span))
        .labelled("a base path such as `/v1`");
    let route = doc()
        .then(ident())
        .then(string())
        .then(ident())
        .then(params(ty.clone()))
        .then(result(ty))
        .then(annotations())
        .map_with_span(
            |((((((doc, method), path), name), params), result), annotations), span| HttpRoute {
                doc,
                method,
                path,
                name,
                params,
                result,
                annotations,
                span,
            },
        );
    kw("http")
        .ignore_then(base.or_not())
        .then(
            route
                .repeated()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .map(|(base, routes)| DeclKind::Http(HttpBlock { base, routes }))
}

fn rpc_decl<T>(ty: T) -> impl Parser<Token, DeclKind, Error = E> + Clone
where
    T: Parser<Token, TypeExpr, Error = E> + Clone,
{
    let method = doc()
        .then(ident())
        .then(params(ty.clone()))
        .then(result(ty))
        .then(annotations())
        .map_with_span(
            |((((doc, name), params), result), annotations), span| RpcMethod {
                doc,
                name,
                params,
                result,
                annotations,
                span,
            },
        );
    kw("rpc")
        .ignore_then(ident())
        .then(
            method
                .repeated()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .map(|(name, methods)| DeclKind::Rpc(RpcBlock { name, methods }))
}

fn graphql_decl<T>(ty: T) -> impl Parser<Token, DeclKind, Error = E> + Clone
where
    T: Parser<Token, TypeExpr, Error = E> + Clone,
{
    let kind = choice((
        kw("query").to(GraphqlKind::Query),
        kw("mutation").to(GraphqlKind::Mutation),
        kw("subscription").to(GraphqlKind::Subscription),
    ))
    .labelled("`query`, `mutation` or `subscription`");
    let field = doc()
        .then(kind)
        .then(ident())
        .then(params(ty.clone()))
        .then(result(ty))
        .then(annotations())
        .map_with_span(
            |(((((doc, kind), name), params), result), annotations), span| GraphqlField {
                doc,
                kind,
                name,
                params,
                result,
                annotations,
                span,
            },
        );
    kw("graphql")
        .ignore_then(
            field
                .repeated()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .map(|fields| DeclKind::Graphql(GraphqlBlock { fields }))
}

fn socket_decl<T>(ty: T) -> impl Parser<Token, DeclKind, Error = E> + Clone
where
    T: Parser<Token, TypeExpr, Error = E> + Clone,
{
    let side = choice((kw("server").to(Side::Server), kw("client").to(Side::Client)))
        .labelled("`server` or `client`");
    let event = doc()
        .then(side)
        .then(path())
        .then_ignore(just(Token::Colon))
        .then(ty)
        .then(annotations())
        .map_with_span(
            |((((doc, side), name), ty), annotations), span| SocketEvent {
                doc,
                side,
                name,
                ty,
                annotations,
                span,
            },
        );
    kw("socket")
        .ignore_then(ident())
        .then(
            event
                .repeated()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .map(|(name, events)| DeclKind::Socket(SocketBlock { name, events }))
}

fn decl() -> impl Parser<Token, Decl, Error = E> + Clone {
    let ty = type_expr();
    let kind = choice((
        enum_decl(),
        type_decl(ty.clone()),
        event_decl(ty.clone()),
        error_decl(ty.clone()),
        http_decl(ty.clone()),
        rpc_decl(ty.clone()),
        graphql_decl(ty.clone()),
        socket_decl(ty),
    ));
    doc()
        .then(kind)
        .map_with_span(|(doc, kind), span| Decl { doc, kind, span })
}

pub(crate) type Parsed = (Package, Vec<Import>, Vec<Decl>);

pub(crate) fn file() -> impl Parser<Token, Parsed, Error = E> {
    let package = kw("package")
        .ignore_then(ident())
        .then(annotations())
        .map_with_span(|(name, annotations), span| Package {
            name,
            annotations,
            span,
        });
    let import = kw("import")
        .ignore_then(ident())
        .map_with_span(|package, span| Import { package, span });
    package
        .then(import.repeated())
        .then(decl().repeated())
        .then_ignore(end())
        .map(|((package, imports), decls)| (package, imports, decls))
}
