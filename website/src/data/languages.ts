/** Supported languages and platforms. Source: docs/languages.md. */
export interface Language {
  name: string;
  icon: string;
  note: string;
}

export const languages: Language[] = [
  { name: "TypeScript", icon: "typescript", note: "Node, Bun, Deno" },
  { name: "Go", icon: "go", note: "backend" },
  { name: "Python", icon: "python", note: "CPython 3.10+" },
  { name: "Kotlin, Java", icon: "kotlin", note: "one JVM package" },
  { name: "C#, F#", icon: "dotnet", note: "one .NET package" },
  { name: "Rust", icon: "rust", note: "Tokio" },
  { name: "Ruby", icon: "ruby", note: "backend" },
  { name: "PHP", icon: "php", note: "8.2+" },
  { name: "Elixir, Erlang", icon: "elixir", note: "BEAM" },
  { name: "Swift", icon: "swift", note: "iOS, macOS, Vapor" },
  { name: "Dart", icon: "dart", note: "Flutter" },
  { name: "Browser", icon: "javascript", note: "client transports" },
];
