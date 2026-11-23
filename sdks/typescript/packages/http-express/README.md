# @weft-sdk/http-express

The HTTP adapter of [Weft](https://github.com/weft-sdk/weft) for TypeScript, on [Express](https://expressjs.com/) 5. It serves the routes of your contracts; Weft does the validation, errors (RFC 9457 problem details), tracing and shutdown. `weft.http.raw` is your Express app.

```sh
npm install @weft-sdk/core @weft-sdk/http-express express
```

Use it through the file `weft gen` writes (`transports.http.library: express` in `weft.yaml`). Needs `@weft-sdk/core` and `express` ^5 as peer dependencies; in TypeScript also `@types/express`.

**Pre-release.** Apache-2.0.
