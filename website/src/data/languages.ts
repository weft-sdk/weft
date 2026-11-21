/**
 * Supported languages, as twelve names (the JVM and .NET families count once each). The detail
 * line for each comes from docs/05-languages.md.
 */
export interface Language {
  name: string;
  icon: string;
  detail: string;
}

/** Shown under the list until the reader points at a language. */
export const languagesIntro =
  "Backend languages run every transport. Browsers, iOS, Android and Flutter use the client-side ones.";

export const languages: Language[] = [
  { name: "TypeScript", icon: "typescript", detail: "Node, Bun and Deno on the backend, and the browser on the client." },
  { name: "JavaScript", icon: "javascript", detail: "Shares the TypeScript package, on Node, Bun and Deno and in the browser." },
  { name: "Go", icon: "go", detail: "Backend, with every transport." },
  { name: "Python", icon: "python", detail: "CPython 3.10 and later." },
  { name: "Java", icon: "java", detail: "Kotlin, Scala and Clojure share the one JVM package. Android uses it too." },
  { name: "C#", icon: "dotnet", detail: "F# shares the one .NET package." },
  { name: "Rust", icon: "rust", detail: "On Tokio." },
  { name: "Ruby", icon: "ruby", detail: "Backend, with every transport." },
  { name: "PHP", icon: "php", detail: "PHP 8.2 and later." },
  { name: "Elixir", icon: "elixir", detail: "Erlang shares it. Both run on the BEAM." },
  { name: "Swift", icon: "swift", detail: "iOS and macOS apps, and the server with Vapor." },
  { name: "Dart", icon: "dart", detail: "Flutter apps, with the client-side transports." },
];
