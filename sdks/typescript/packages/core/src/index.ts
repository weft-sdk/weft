export { Contracts, named, type ContractsOptions } from "./contracts.js"
export {
  CATEGORIES,
  WeftError,
  isWeftError,
  type CategoryInfo,
  type ErrorCategory,
  type Issue,
  type MessagingPolicy,
  type WeftErrorOptions,
} from "./errors.js"
export { checkFormat } from "./formats.js"
export * from "./ir.js"

/** The version of this package. Replaced by the release process. */
export const version = "0.0.0"
