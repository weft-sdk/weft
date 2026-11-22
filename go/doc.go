// Package weft is Weft for Go: the weft.* runtime that reads the checked contracts (the IR) and
// validates, encodes and decodes values, runs the lifecycle, and talks over HTTP and messaging.
//
// This file's package is the core. Adapters for libraries are packages inside this module.
// The behaviour every language shares is written in spec/behaviour.md; the conformance suite
// checks it.
package weft

// Version is the version of this package. It is replaced by the release process.
const Version = "0.0.0"
