/**
 * The contract section: one contract and the types `weft gen` writes from it, in five languages.
 * The contract is the one in docs/02-contract-language.md. The generated types follow its type
 * mapping table (uuid, timestamp, optional fields, arrays) and the naming per language:
 * camelCase in TypeScript, PascalCase fields in Go, snake_case in Python and Rust. Java is a
 * record, and its optional field uses a @Nullable annotation. Each block is one entry per line.
 */
const esc = (x: string) => x.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
const k = (x: string) => `<span class="tok-k">${esc(x)}</span>`;
const a = (x: string) => `<span class="tok-a">${esc(x)}</span>`;

export interface Column {
  id: string;
  name: string;
  sub: string;
  /** An Icon name. */
  icon: string;
  lines: string[];
}

/** The contract, in two halves of eight lines each so the strip reads evenly. */
export const contract = {
  left: [
    `${k("type")} Order {`,
    `  id: uuid`,
    `  customerId: string`,
    `  items: OrderItem[]`,
    `  total: Money`,
    `  note?: string`,
    `  placedAt: timestamp`,
    `}`,
  ],
  right: [
    `${k("type")} Money {`,
    `  amount: int            ${k("// minor units")}`,
    `  currency: Currency`,
    `}`,
    ` `,
    `${k("event")} ${a("order.placed")}: Order {`,
    `  key: id`,
    `}`,
  ],
};

/** The same type as `weft gen` writes it, one column per language. */
export const columns: Column[] = [
  {
    id: "ts",
    name: "TypeScript",
    sub: "interface",
    icon: "typescript",
    lines: [
      `${k("export interface")} Order {`,
      `  id: Uuid`,
      `  customerId: string`,
      `  items: OrderItem[]`,
      `  total: Money`,
      `  note?: string`,
      `  placedAt: Date`,
      `}`,
    ],
  },
  {
    id: "go",
    name: "Go",
    sub: "struct",
    icon: "go",
    lines: [
      `${k("type")} Order ${k("struct")} {`,
      `    ID         uuid.UUID`,
      `    CustomerID string`,
      `    Items      []OrderItem`,
      `    Total      Money`,
      `    Note       *string`,
      `    PlacedAt   time.Time`,
      `}`,
    ],
  },
  {
    id: "py",
    name: "Python",
    sub: "dataclass",
    icon: "python",
    lines: [
      `${k("@dataclass")}`,
      `${k("class")} Order:`,
      `    id: UUID`,
      `    customer_id: str`,
      `    items: ${esc("list[OrderItem]")}`,
      `    total: Money`,
      `    placed_at: datetime`,
      `    note: str | None = None`,
    ],
  },
  {
    id: "rs",
    name: "Rust",
    sub: "struct",
    icon: "rust",
    lines: [
      `${k("pub struct")} Order {`,
      `    ${k("pub")} id: Uuid,`,
      `    ${k("pub")} customer_id: String,`,
      `    ${k("pub")} items: ${esc("Vec<OrderItem>")},`,
      `    ${k("pub")} total: Money,`,
      `    ${k("pub")} note: ${esc("Option<String>")},`,
      `    ${k("pub")} placed_at: ${esc("DateTime<Utc>")},`,
      `}`,
    ],
  },
  {
    id: "java",
    name: "Java",
    sub: "record",
    icon: "java",
    lines: [
      `${k("public record")} Order(`,
      `    UUID id,`,
      `    String customerId,`,
      `    ${esc("List<OrderItem>")} items,`,
      `    Money total,`,
      `    ${k("@Nullable")} String note,`,
      `    Instant placedAt) {}`,
    ],
  },
];
