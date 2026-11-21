//! The syntax tree. Every node carries the span it was parsed from.

use crate::token::{Comment, Span};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    pub package: Package,
    pub imports: Vec<Import>,
    pub decls: Vec<Decl>,
    /// Every `//` comment, in order, for the formatter.
    pub comments: Vec<Comment>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

/// A dotted name such as `order.placed` or `orders.Order`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Path {
    pub segments: Vec<Ident>,
    pub span: Span,
}

impl Path {
    pub fn text(&self) -> String {
        self.segments
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>()
            .join(".")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Package {
    pub name: Ident,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Import {
    pub package: Ident,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnnotationArg {
    Str(String),
    /// A number as written.
    Num(String),
    Ident(String),
}

/// `@name` or `@name(arg, ...)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Annotation {
    pub name: Ident,
    pub args: Vec<AnnotationArg>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decl {
    /// Lines of `///` documentation.
    pub doc: Vec<String>,
    pub kind: DeclKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeclKind {
    Enum(EnumDecl),
    Type(TypeDecl),
    Event(EventDecl),
    Error(ErrorDecl),
    Http(HttpBlock),
    Rpc(RpcBlock),
    Graphql(GraphqlBlock),
    Socket(SocketBlock),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnumDecl {
    pub name: Ident,
    pub annotations: Vec<Annotation>,
    pub variants: Vec<Variant>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variant {
    pub doc: Vec<String>,
    pub name: Ident,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeDecl {
    pub name: Ident,
    pub params: Vec<Ident>,
    pub annotations: Vec<Annotation>,
    pub fields: Vec<Field>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErrorDecl {
    pub name: Ident,
    pub annotations: Vec<Annotation>,
    pub fields: Vec<Field>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub doc: Vec<String>,
    pub name: Ident,
    /// `name?: T`
    pub optional: bool,
    pub ty: TypeExpr,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeExpr {
    pub kind: TypeKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeKind {
    /// `Order`, `orders.Order`, `Page<Order>`, `map<string, int>`.
    Named { path: Path, args: Vec<TypeExpr> },
    /// `T[]`
    List(Box<TypeExpr>),
    /// `{ id: uuid, reason: string }`
    Inline(Vec<Field>),
    /// `T?`. Only valid in a result.
    Optional(Box<TypeExpr>),
    /// `A | B`. Only valid in a result.
    Union(Vec<TypeExpr>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventDecl {
    pub name: Path,
    pub ty: TypeExpr,
    pub options: Vec<EventOption>,
    pub annotations: Vec<Annotation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventOption {
    /// `key: id`
    Key { path: Path, span: Span },
    /// `headers: { traceparent: string }`
    Headers { ty: TypeExpr, span: Span },
}

/// A parameter of an operation: `id: uuid`, `page?: int`, or an unnamed type as in `rpc` methods.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Param {
    pub name: Option<Ident>,
    pub optional: bool,
    /// `stream T`, for client streaming in `rpc`.
    pub stream: bool,
    pub ty: TypeExpr,
    pub span: Span,
}

/// What follows `->`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResultType {
    pub stream: bool,
    pub ty: TypeExpr,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpBlock {
    /// `/v1` in `http /v1 { ... }`.
    pub base: Option<(String, Span)>,
    pub routes: Vec<HttpRoute>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpRoute {
    pub doc: Vec<String>,
    pub method: Ident,
    pub path: (String, Span),
    pub name: Ident,
    pub params: Vec<Param>,
    pub result: ResultType,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RpcBlock {
    pub name: Ident,
    pub methods: Vec<RpcMethod>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RpcMethod {
    pub doc: Vec<String>,
    pub name: Ident,
    pub params: Vec<Param>,
    pub result: ResultType,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphqlBlock {
    pub fields: Vec<GraphqlField>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphqlKind {
    Query,
    Mutation,
    Subscription,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphqlField {
    pub doc: Vec<String>,
    pub kind: GraphqlKind,
    pub name: Ident,
    pub params: Vec<Param>,
    pub result: ResultType,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SocketBlock {
    pub name: Ident,
    pub events: Vec<SocketEvent>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Server,
    Client,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SocketEvent {
    pub doc: Vec<String>,
    pub side: Side,
    pub name: Path,
    pub ty: TypeExpr,
    pub annotations: Vec<Annotation>,
    pub span: Span,
}
